#!/usr/bin/env bash
set -euo pipefail

# Import only the reviewed v0.2.0 source archive. The local source checkout is
# an import input, never a durable Cargo or Nix dependency.
source_commit=a87ca01412fb62e69171925d79d2d6b0a23a7e72
archive_blake3=7c2902aee85ebcc923b1e7f9004d6235b550ddf18053194dd6c9286ef05859e6

if (( $# != 1 )); then
  printf 'usage: %s <local-shared-source-checkout>\n' "$0" >&2
  exit 2
fi

source_root=$1
repo_root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
destination="$repo_root/vendor/radicle-cob-git-store"
source_record="$repo_root/vendor/radicle-cob-git-store.source.json"
if [[ "$(jq -r '.source_commit' "$source_record")" != "$source_commit" ]] ||
   [[ "$(jq -r '.archive_blake3' "$source_record")" != "$archive_blake3" ]]; then
  printf 'shared COB source record disagrees with importer pin\n' >&2
  exit 1
fi
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

git -C "$source_root" cat-file -e "${source_commit}^{commit}"
git -C "$source_root" archive --format=tar "$source_commit" > "$temporary/source.tar"
observed=$(b3sum --no-names "$temporary/source.tar")
if [[ "$observed" != "$archive_blake3" ]]; then
  printf 'shared COB source archive digest mismatch: %s\n' "$observed" >&2
  exit 1
fi
mkdir "$temporary/source"
tar -xf "$temporary/source.tar" -C "$temporary/source"
if [[ -e "$destination" || -L "$destination" ]]; then
  # Never overwrite an existing snapshot: compare bytes, symlink targets and
  # executable bits before any future immutable source pin can be changed.
  git diff --no-index --exit-code -- "$temporary/source" "$destination"
  printf 'verified existing vendored COB source: %s\n' "$source_commit"
  exit 0
fi
mkdir -p "$(dirname "$destination")"
mv --no-clobber -T -- "$temporary/source" "$destination"
if [[ -e "$temporary/source" ]]; then
  printf 'shared COB vendor destination appeared during import; refusing replacement\n' >&2
  exit 1
fi
printf 'imported vendored COB source: %s\n' "$source_commit"
