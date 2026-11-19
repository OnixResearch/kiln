#!/usr/bin/env bash
set -euo pipefail

# A checkout is only an import input. Read the pinned Git object, never its
# mutable working tree. Runtime Cargo/Nix acquisition uses the committed bytes.
if (( $# != 2 )); then
  printf 'usage: %s <lattice|cairn|cairn-verifier|valence|artifact|bounded-exec|mantle|ucan|basalt|subwayrat> <source-checkout>\n' "$0" >&2
  exit 2
fi
name=$1
source_root=$2
case "$name" in
  lattice)
    source_commit=70496e67c7fd4a8b05914161a8e09de2759bebc8
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  cairn)
    source_commit=15f00875562025e7ea7e0d1f4af24d1a2e2ac06f
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  cairn-verifier)
    source_commit=15f00875562025e7ea7e0d1f4af24d1a2e2ac06f
    archive_blake3=068e403844cbbd718e4ced8d0848b84cbfc7eff66d601a8a85b5231f747f5e3d ;;
  valence)
    source_commit=220307a371e7bc4e3309447b09447d0fe6c4db5f
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  artifact)
    source_commit=e41340bec587b6d049b5cc518ec7db925dde84be
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  bounded-exec)
    source_commit=29dac88ecded94457572db3fdfaaaab95fa91525
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  mantle)
    source_commit=d75158b00b080535f2ffea12430665ca3d57ea82
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  ucan)
    source_commit=417f74662b9de2de3bdabe93ad5db996e1bd2861
    archive_blake3=3a35ee1e0c6db623109e096e563c7ba9c02bb4d406bf72cae6010bdb7fe54fbc ;;
  basalt)
    source_commit=264d248912e58760b5ae1eced51b7c7d50eb5a96
    archive_blake3=2d8e91ab7681f2b354fd99308e7c810f2dfba628afb32deb2228f5953795027d ;;
  subwayrat)
    source_commit=2e52b3150819a2365aaefd3dcf8bbd2a2fa2e901
    archive_blake3=2e6588dd7b182c5a689c5e7be42f67fb09a5aa806449709f05066f6004e5f8d7 ;;
  *) printf 'unknown source import: %s\n' "$name" >&2; exit 2 ;;
esac
repo_root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
destination="$repo_root/vendor/$name"
source_record="$repo_root/vendor/$name.source.json"
if [[ "$(jq -r '.source_commit' "$source_record")" != "$source_commit" ]] ||
   [[ "$(jq -r '.archive_blake3' "$source_record")" != "$archive_blake3" ]]; then
  printf '%s source record disagrees with importer pin\n' "$name" >&2
  exit 1
fi
mapfile -t archive_paths < <(jq -r '.archive_paths[]' "$source_record")
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

git -C "$source_root" cat-file -e "${source_commit}^{commit}"
git -C "$source_root" archive --format=tar "$source_commit" -- "${archive_paths[@]}" > "$temporary/source.tar"
observed=$(b3sum --no-names "$temporary/source.tar")
if [[ "$observed" != "$archive_blake3" ]]; then
  printf '%s source archive digest mismatch: %s\n' "$name" "$observed" >&2
  exit 1
fi
mkdir "$temporary/source"
tar -xf "$temporary/source.tar" -C "$temporary/source"
# A partial import retains the upstream workspace manifest for package
# inheritance. Drop members whose imported paths are absent so Cargo can read
# the manifest without resolving unselected upstream producers.
# Pinned upstream bytes are also outside the host workspace formatter. Every
# rustfmt configuration root in the imported tree disables formatting, so the
# direct workspace formatter covers Kiln-owned sources without rewriting
# vendored code.
python3 - "$temporary/source" <<'PY'
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
manifest = root / "Cargo.toml"
if manifest.is_file():
    text = manifest.read_text()
    match = re.search(r"(?m)^members\s*=\s*\[(.*?)\]", text, re.S)
    if match:
        body = match.group(1)
        entries = re.findall(r'"([^"]+)"', body)
        keep = [entry for entry in entries if (root / entry).is_dir()]
        if len(keep) != len(entries):
            first_indent = re.search(r"(?m)^([ \t]*)\"", body)
            indent = first_indent.group(1) if first_indent else "  "
            replacement = "members = [\n" + "".join(
                f'{indent}"{entry}",\n' for entry in keep
            ) + "]"
            manifest.write_text(text[: match.start()] + replacement + text[match.end() :])
policy = "disable_all_formatting = true"
configs = list(root.rglob("rustfmt.toml"))
if not any(config.parent == root for config in configs):
    configs.append(root / "rustfmt.toml")
for config in configs:
    config.write_text(policy + "\n")
PY
# Apply only the literal manifest rewrites recorded in the source record. Each
# rewrite names the exact field bytes, the expected occurrence count and the
# reason; drift refuses instead of inferring a replacement.
python3 - "$temporary/source" "$source_record" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
record = json.loads(pathlib.Path(sys.argv[2]).read_text())
for rewrite in record.get("manifest_rewrites", []):
    target = root / rewrite["path"]
    text = target.read_text()
    expected = rewrite["occurrences"]
    observed = text.count(rewrite["from"])
    if observed != expected:
        raise SystemExit(
            f'{rewrite["path"]}: expected {expected} occurrence(s) of {rewrite["from"]!r}, found {observed}'
        )
    target.write_text(text.replace(rewrite["from"], rewrite["to"]))
PY
if [[ -e "$destination" || -L "$destination" ]]; then
  # Compare bytes, symlink targets and executable bits. Never replace drift.
  git diff --no-index --exit-code -- "$temporary/source" "$destination"
  printf 'verified existing vendored %s source: %s\n' "$name" "$source_commit"
  exit 0
fi
mkdir -p "$(dirname "$destination")"
mv --no-clobber -T -- "$temporary/source" "$destination"
if [[ -e "$temporary/source" ]]; then
  printf '%s vendor destination appeared during import; refusing replacement\n' "$name" >&2
  exit 1
fi
printf 'imported vendored %s source: %s\n' "$name" "$source_commit"
