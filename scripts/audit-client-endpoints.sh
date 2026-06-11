#!/usr/bin/env bash
#
# Diff the built-in crates.io client's endpoints against the live crates.io
# OpenAPI spec, to catch API drift (paths that no longer match the registry).
#
# This is the check that surfaced the /trustpub -> /trusted_publishing rename
# and the owner-invitation accept path bug (issue #138): the wiremock tests pin
# the client's *own* paths, so only a diff against the authoritative spec can
# catch contract drift. Run it periodically.
#
# Informational only -- prints findings, never fails. Some "in spec, not
# implemented" entries are intentionally out of scope (account-email ops, the
# .crate file download); some "client, not in spec" entries are real private
# endpoints the spec simply doesn't document (/me/*, follow).
#
# Usage:  ./scripts/audit-client-endpoints.sh
# Needs:  curl, jq

set -euo pipefail
cd "$(dirname "$0")/.."

SPEC_URL="https://crates.io/api/openapi.json"
spec="$(mktemp)"
trap 'rm -f "$spec" "$spec.spec" "$spec.client"' EXIT

curl -sf -H "User-Agent: cratesio-mcp-endpoint-audit" "$SPEC_URL" -o "$spec"

# Spec paths: strip the /api/v1 prefix and normalize {param} -> {}.
jq -r '.paths | keys[]' "$spec" \
  | sed -E 's#^/api/v1##; s#\{[^}]+\}#{}#g' | sort -u > "$spec.spec"

# Client paths: pull path-string literals from the endpoint modules and
# normalize the same way.
grep -rhoE '"/[a-zA-Z0-9_./{}-]+"' \
  src/client/{crates,versions,owners,categories,keywords,users,teams,tokens,trusted,publish,metadata}.rs \
  | tr -d '"' | sed -E 's#\{[^}]+\}#{}#g' | grep -E '^/' | sort -u > "$spec.client"

echo "spec: $(wc -l < "$spec.spec" | tr -d ' ') paths    client: $(wc -l < "$spec.client" | tr -d ' ') paths"
echo
echo "## In spec but NOT implemented by the client (gaps; some intentionally out of scope):"
comm -23 "$spec.spec" "$spec.client" | sed 's/^/  /' || true
echo
echo "## Client paths NOT in the spec (renamed/deprecated, or undocumented private endpoints -- review):"
comm -13 "$spec.spec" "$spec.client" | sed 's/^/  /' || true
