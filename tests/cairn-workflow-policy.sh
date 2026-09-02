#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

cairn policy export --source cairn-policy/default.ncl \
  --output cairn-policy/generated/cairn-policy.json --check
cairn gate proposal prepare-public-release --root . |
  jq -e '.valid == true and .verdict == "PASS" and .workflow_profile == "spec-driven" and .workflow_profile_compatibility_fallback == false' >/dev/null

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
nickel export cairn-policy/fixtures/negative-missing-outcome-profile.ncl >"$tmp/missing-outcome.json"
if cairn gate proposal prepare-public-release --root . \
  --policy "$tmp/missing-outcome.json" >"$tmp/output" 2>"$tmp/error"; then
  printf 'policy without outcome-machine passed a Cairn gate\n' >&2
  exit 1
fi
error=$(<"$tmp/error")
if [[ "$error" != *'initial workflow profile is missing: outcome-machine'* ]]; then
  printf 'missing outcome-machine profile failed for a different reason: %s\n' "$error" >&2
  exit 1
fi
printf 'Cairn accepts the selected spec-driven profile and denies missing outcome-machine\n'
