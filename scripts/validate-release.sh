#!/usr/bin/env bash
set -euo pipefail

endpoint="${MCP_ENDPOINT:-https://cratesio-mcp.fly.dev/}"
repl="${MCP_REPL_BIN:-mcp-repl}"
max_attempts="${MCP_VALIDATION_ATTEMPTS:-3}"
retry_delay="${MCP_VALIDATION_RETRY_DELAY_SECS:-5}"
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

validate_tools() {
  local protocol="$1"
  local tools_json

  if ! tools_json=$("$repl" --protocol "$protocol" --http "$endpoint" --json -e tools); then
    echo "mcp-repl failed using the $protocol lifecycle" >&2
    return 1
  fi

  if ! jq -e '
    any(.[];
      .name == "search_crates"
      and .outputSchema.type == "object"
      and (.outputSchema.required | index("crates") != null)
      and (.outputSchema.required | index("meta") != null)
      and .annotations.readOnlyHint == true
      and .annotations.idempotentHint == true
    )
  ' >/dev/null <<<"$tools_json"; then
    echo "search_crates is missing its expected schema or annotations ($protocol)" >&2
    jq . <<<"$tools_json" >&2
    return 1
  fi
}

validate_tool_result() {
  local result_json

  if ! result_json=$(
    "$repl" --protocol 2026-07-28 --http "$endpoint" --json \
      -e "search_crates query=tower-mcp"
  ); then
    echo "search_crates failed using the final lifecycle" >&2
    return 1
  fi

  if ! jq -e '
    (.isError // false) == false
    and (.structuredContent.crates | type) == "array"
    and (.structuredContent.crates | length) > 0
    and any(.structuredContent.crates[]; .name == "tower-mcp")
    and .structuredContent.meta.total > 0
    and any(.content[]; .type == "text" and (.text | contains("tower-mcp")))
  ' >/dev/null <<<"$result_json"; then
    echo "search_crates returned an unexpected tool result" >&2
    jq . <<<"$result_json" >&2
    return 1
  fi

  if [[ -n "${EXPECTED_VERSION:-}" ]]; then
    local expected_version="${EXPECTED_VERSION#v}"
    if ! jq -e --arg expected "$expected_version" '
      ._meta["io.modelcontextprotocol/serverInfo"].version == $expected
    ' >/dev/null <<<"$result_json"; then
      echo "live server version does not match release $EXPECTED_VERSION" >&2
      jq '._meta' <<<"$result_json" >&2
      return 1
    fi
  fi

  jq '{
    server: ._meta["io.modelcontextprotocol/serverInfo"],
    matchedCrates: [.structuredContent.crates[].name],
    total: .structuredContent.meta.total
  }' <<<"$result_json"
}

validate_subscription_concurrency() {
  if ! node "$script_dir/validate-subscription.mjs" "$endpoint"; then
    echo "official client could not list tools with subscriptions/listen open" >&2
    return 1
  fi
}

for attempt in $(seq 1 "$max_attempts"); do
  echo "MCP release validation attempt $attempt/$max_attempts"

  if validate_tools stable \
    && validate_tools 2026-07-28 \
    && validate_subscription_concurrency \
    && validate_tool_result; then
    echo "MCP release validation passed"
    exit 0
  fi

  if [[ "$attempt" -lt "$max_attempts" ]]; then
    echo "Retrying in ${retry_delay}s..." >&2
    sleep "$retry_delay"
  fi
done

echo "MCP release validation failed after $max_attempts attempts" >&2
exit 1
