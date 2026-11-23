#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

# Positive: all pinned digests match their vendor/*.source.json and are unique.
bash scripts/import-pinned-source.sh --check-pins

# Negative: duplicate archive_blake3 across distinct sources must be refused.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/scripts" "$tmp/vendor"
cp scripts/import-pinned-source.sh "$tmp/scripts/import-pinned-source.sh"
cp vendor/*.source.json "$tmp/vendor/"

mantle_digest=$(jq -r '.archive_blake3' "$tmp/vendor/mantle.source.json")
jq --arg digest "$mantle_digest" '.archive_blake3 = $digest' \
  "$tmp/vendor/lattice.source.json" >"$tmp/vendor/lattice.source.json.tmp"
mv "$tmp/vendor/lattice.source.json.tmp" "$tmp/vendor/lattice.source.json"
python3 - "$tmp/scripts/import-pinned-source.sh" "$mantle_digest" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
digest = sys.argv[2]
text = p.read_text()
text = re.sub(
    r"(lattice\)\s*\n\s*source_commit=[0-9a-f]+\s*\n\s*archive_blake3=)[0-9a-f]+",
    rf"\g<1>{digest}",
    text,
    count=1,
)
p.write_text(text)
PY

if bash "$tmp/scripts/import-pinned-source.sh" --check-pins >"$tmp/out" 2>"$tmp/err"; then
  printf 'importer pin check accepted a duplicate digest across distinct sources\n' >&2
  exit 1
fi
err=$(<"$tmp/err")
if [[ "$err" != *'duplicate archive_blake3'* ]]; then
  printf 'unexpected duplicate-digest failure: %s\n' "$err" >&2
  exit 1
fi
printf 'importer pin check verified positive pins and refused duplicate digest\n'
