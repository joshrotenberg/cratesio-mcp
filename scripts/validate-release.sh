#!/usr/bin/env bash
set -euo pipefail

endpoint="${MCP_ENDPOINT:-https://cratesio-mcp.fly.dev/}"
repl="${MCP_REPL_BIN:-mcp-repl}"
max_attempts="${MCP_VALIDATION_ATTEMPTS:-3}"
retry_delay="${MCP_VALIDATION_RETRY_DELAY_SECS:-5}"
command_timeout="${MCP_VALIDATION_TIMEOUT_SECS-30}"
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

# Zero disables GNU timeout, so reject it (and malformed values) before running
# any probes. Each subprocess has its own deadline, including client shutdown.
if [[ ! "$command_timeout" =~ ^[1-9][0-9]*$ ]]; then
  echo "MCP_VALIDATION_TIMEOUT_SECS must be a positive integer" >&2
  exit 1
fi

if command -v timeout >/dev/null 2>&1; then
  timeout_bin=timeout
elif command -v gtimeout >/dev/null 2>&1; then
  timeout_bin=gtimeout
else
  echo "GNU timeout is required (install coreutils on macOS)" >&2
  exit 1
fi

run_probe() {
  local phase="$1"
  shift
  local status

  # Keep stdout clean for the callers that capture JSON. Without --foreground,
  # timeout also signals the process group; --kill-after bounds TERM-resistant
  # clients. The job timeout remains a final guard, not the retry mechanism.
  echo "Validating $phase (timeout ${command_timeout}s)" >&2
  if "$timeout_bin" --kill-after=5s "${command_timeout}s" "$@"; then
    return 0
  else
    status=$?
  fi

  case "$status" in
    124)
      echo "$phase timed out after ${command_timeout}s" >&2
      ;;
    137)
      # timeout itself is killed with its process group after the grace period;
      # a child killed independently (for example by OOM) has the same status.
      echo "$phase timed out or was killed (deadline ${command_timeout}s + 5s cleanup grace; exit $status)" >&2
      ;;
    *)
      echo "$phase failed (exit $status)" >&2
      ;;
  esac
  return "$status"
}

validate_tools() {
  local protocol="$1"
  local tools_json

  if ! tools_json=$(run_probe "tools/list ($protocol)" \
    "$repl" --protocol "$protocol" --http "$endpoint" --json -e tools); then
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
    run_probe "search_crates (2026-07-28)" \
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
  # Asserts that subscriptions/listen opens an SSE stream, that tools/list
  # completes while it is held open, and that a real search_crates call
  # returns a sane result. It prints which one failed, so this message does
  # not guess. The uptime probe runs this same script directly.
  if ! run_probe "deployment probe" node "$script_dir/validate-deployment.mjs" "$endpoint"; then
    echo "deployment validation failed; see the error above" >&2
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
