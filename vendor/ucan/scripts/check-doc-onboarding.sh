#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
readme="$repo_root/README.md"

line_for_heading() {
  local heading=$1
  local line
  line=$(grep -nFx "$heading" "$readme" | cut -d: -f1 || true)
  if [[ -z "$line" ]]; then
    echo "missing README heading: $heading" >&2
    return 1
  fi
  printf '%s\n' "$line"
}

architecture_line=$(line_for_heading '## Architecture')
flow_line=$(line_for_heading '## End-to-end issue/parse/verify')
proof_line=$(line_for_heading '## Proof-chain delegation')
support_line=$(line_for_heading '## Support limits')

if ! [[ "$architecture_line" -lt "$flow_line" && "$flow_line" -lt "$proof_line" && "$proof_line" -lt "$support_line" ]]; then
  echo 'README onboarding order must be architecture, end-to-end flow, proof-chain deep dive, support limits' >&2
  exit 1
fi

for required in \
  'Supported DID methods' \
  'Supported signing algorithms' \
  'Proof-loading model' \
  'Out-of-scope UCAN features' \
  'did:key' \
  'Ed25519' \
  'caller-supplied proof tokens'; do
  if ! grep -Fq "$required" "$readme"; then
    echo "README missing support-limit text: $required" >&2
    exit 1
  fi
done
