#!/usr/bin/env bash
set -euo pipefail

# A checkout is only an import input. Read the pinned Git object, never its
# mutable working tree. Runtime Cargo/Nix acquisition uses the committed bytes.
repo_root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")

check_pins() {
  local script_path=$1
  python3 - "$script_path" "$repo_root" <<'PY'
import json
import pathlib
import re
import sys

script_path = pathlib.Path(sys.argv[1])
repo_root = pathlib.Path(sys.argv[2])
text = script_path.read_text()
pattern = re.compile(
    r"^\s*([a-z0-9-]+)\)\s*\n\s*source_commit=([0-9a-f]+)\s*\n\s*archive_blake3=([0-9a-f]+)\s*;;",
    re.MULTILINE,
)
pins = pattern.findall(text)
if not pins:
    raise SystemExit("no pins found in importer script")

seen_digests = {}
for name, source_commit, archive_blake3 in pins:
    record_path = repo_root / "vendor" / f"{name}.source.json"
    if not record_path.is_file():
        raise SystemExit(f"{name}: missing source record {record_path}")
    record = json.loads(record_path.read_text())
    if record.get("source_commit") != source_commit or record.get("archive_blake3") != archive_blake3:
        raise SystemExit(f"{name}: source record disagrees with importer pin")
    identity = (
        record.get("source_repository"),
        record.get("source_commit"),
        tuple(record.get("archive_paths", [])),
    )
    previous = seen_digests.get(archive_blake3)
    if previous is not None and previous[1] != identity:
        raise SystemExit(
            f"duplicate archive_blake3 {archive_blake3} shared by {previous[0]} and {name} with distinct sources"
        )
    seen_digests[archive_blake3] = (name, identity)
PY
}

if (( $# == 1 )) && [[ "$1" == "--check-pins" ]]; then
  check_pins "${BASH_SOURCE[0]}"
  printf 'verified importer pins match vendor source records with unique archive digests\n'
  exit 0
fi

if (( $# != 2 )); then
  printf 'usage: %s <lattice|cairn|cairn-verifier|cairn-kiln-core|valence|artifact|bounded-exec|mantle|ucan|basalt|subwayrat> <source-checkout>\n' "$0" >&2
  printf '   or: %s --check-pins\n' "$0" >&2
  exit 2
fi
name=$1
source_root=$2
case "$name" in
  lattice)
    source_commit=70496e67c7fd4a8b05914161a8e09de2759bebc8
    archive_blake3=65c2190700bf8a5e6760116267935b151082be97ed2b0a2b62036e4917c0392d ;;
  cairn)
    source_commit=317d3ff648f01a5ea1572e6126f4dd71bc7afa99
    archive_blake3=f966f2a01a209a0d16954c4976eae0710076281d7bbb52ddfb8f63072fe51860 ;;
  cairn-verifier)
    source_commit=317d3ff648f01a5ea1572e6126f4dd71bc7afa99
    archive_blake3=445f6e2919e65eef63db85d63e4c52e2a053ec2f86442856756c89ed08b6bf9a ;;
  cairn-kiln-core)
    source_commit=5d99f4e9bb954743443e340baece31256b4c53f1
    archive_blake3=c99e9f1d5dfec309187fa7f807062fd5da6f014a65188ea51ccecb0f6f96097e ;;
  valence)
    source_commit=220307a371e7bc4e3309447b09447d0fe6c4db5f
    archive_blake3=219c641cbdc074054809bbc906b9cfce853c0691ea8b4119ca654f1a0c520572 ;;
  artifact)
    source_commit=e41340bec587b6d049b5cc518ec7db925dde84be
    archive_blake3=737362f0b98f8aa4fef2aa5727b2e747136648d24ccbe4f3f903e7679d50a77d ;;
  bounded-exec)
    source_commit=29dac88ecded94457572db3fdfaaaab95fa91525
    archive_blake3=d75d4f6f0df36c6b70080a37c0a88d5319035097aa4aecd0fab7cd25e571c33d ;;
  mantle)
    source_commit=d75158b00b080535f2ffea12430665ca3d57ea82
    archive_blake3=56120c08a7d04520b2af8926bec445bc23e69d69ac5e4a53131777d8d72e5083 ;;
  ucan)
    source_commit=fca62a6ed6895689ff18112b497bc7a986516c20
    archive_blake3=55c68cddbaa7dc62bbb9c65ba254917911bda54ce82436534423d1a9f85b6722 ;;
  basalt)
    source_commit=fac0d7f1fd7ac936ca4d3f251f3287666e93d4bb
    archive_blake3=8f5d3c4b89d7309c74c49707669e3ad163c0bce606b919df1eaf733e8bd47ecb ;;
  subwayrat)
    source_commit=2e52b3150819a2365aaefd3dcf8bbd2a2fa2e901
    archive_blake3=2e6588dd7b182c5a689c5e7be42f67fb09a5aa806449709f05066f6004e5f8d7 ;;
  *) printf 'unknown source import: %s\n' "$name" >&2; exit 2 ;;
esac
check_pins "${BASH_SOURCE[0]}"
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
  # r[impl kiln.implementation.authority.cohort]
  # The recorded cohort is verified by crates/kiln-adapter-basalt/tests/cohort.rs.
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
