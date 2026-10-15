#!/usr/bin/env bash
set -euo pipefail

# A checkout is only an import input. Read the pinned Git object, never its
# mutable working tree. Runtime Cargo/Nix acquisition uses the committed bytes.
if (( $# != 2 )); then
  printf 'usage: %s <lattice|cairn|valence|artifact|bounded-exec|mantle> <source-checkout>\n' "$0" >&2
  exit 2
fi
name=$1
source_root=$2
case "$name" in
  lattice)
    source_commit=70496e67c7fd4a8b05914161a8e09de2759bebc8
    archive_blake3=65c2190700bf8a5e6760116267935b151082be97ed2b0a2b62036e4917c0392d ;;
  cairn)
    source_commit=15f00875562025e7ea7e0d1f4af24d1a2e2ac06f
    archive_blake3=fe983828dde8c99867f3479ca561fa2b88ffb3a4190bbea61a14ca3985ee6a0a ;;
  valence)
    source_commit=220307a371e7bc4e3309447b09447d0fe6c4db5f
    archive_blake3=219c641cbdc074054809bbc906b9cfce853c0691ea8b4119ca654f1a0c520572 ;;
  artifact)
    source_commit=e41340bec587b6d049b5cc518ec7db925dde84be
    archive_blake3=40817d2709fd99888a8aae951fca1c233c4a105a2f292e82ee755a987e54f109 ;;
  bounded-exec)
    source_commit=29dac88ecded94457572db3fdfaaaab95fa91525
    archive_blake3=d75d4f6f0df36c6b70080a37c0a88d5319035097aa4aecd0fab7cd25e571c33d ;;
  mantle)
    source_commit=0bbab77e23ab95caa4e4184b9945cfaa5c8040e2
    archive_blake3=ff7c4a11d08625a977b4139367dc619fab5544f54c492080107f20e7e3c42bf5 ;;
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
