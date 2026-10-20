#!/usr/bin/env bash
set -euo pipefail

# Import the published Git object, never mutable checkout bytes.
if (( $# != 1 )); then
  printf 'usage: %s <onixos-source-checkout>\n' "$0" >&2
  exit 2
fi
source_commit=69a131d71a509f9c57646b71113f75f39ab637b1
source_path=fixtures/kiln-wasm-discovery/flake.nix
source_blake3=dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb
repo_root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
destination="$repo_root/runtime/onix-eval-cohort/jobset/flake.nix"
receipt="$repo_root/runtime/onix-eval-cohort/jobset.source.json"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

git -C "$1" show "$source_commit:$source_path" > "$temporary/flake.nix"
observed=$(b3sum --no-names "$temporary/flake.nix")
if [[ "$observed" != "$source_blake3" ]]; then
  printf 'Onix discovery fixture digest mismatch: %s\n' "$observed" >&2
  exit 1
fi
cat > "$temporary/jobset.source.json" <<EOF
{
  "source_repository": "ssh://git@github.com/onixcomputer/onix-modules.git",
  "source_commit": "$source_commit",
  "source_path": "$source_path",
  "source_blake3": "$source_blake3",
  "source_license": "AGPL-3.0-or-later",
  "importer": "scripts/import-onix-discovery-fixture.sh",
  "scope": "One unmodified, commit-bound discovery-only flake; no sibling checkout is required to build Kiln.",
  "nonclaims": "Import identity is not source trust, remote publication, build success, deployment safety, or release eligibility."
}
EOF
if [[ -e "$receipt" || -L "$receipt" ]]; then
  cmp -- "$temporary/jobset.source.json" "$receipt"
fi
if [[ -e "$destination" || -L "$destination" ]]; then
  cmp -- "$temporary/flake.nix" "$destination"
else
  mkdir -p "$(dirname "$destination")"
  mv --no-clobber -T -- "$temporary/flake.nix" "$destination"
  if [[ -e "$temporary/flake.nix" ]]; then
    printf 'Onix fixture destination appeared during import; refusing replacement\n' >&2
    exit 1
  fi
fi
if [[ ! -e "$receipt" && ! -L "$receipt" ]]; then
  mv --no-clobber -T -- "$temporary/jobset.source.json" "$receipt"
  if [[ -e "$temporary/jobset.source.json" ]]; then
    printf 'Onix fixture receipt appeared during import; refusing replacement\n' >&2
    exit 1
  fi
fi
printf 'verified immutable Onix discovery fixture: %s\n' "$source_commit"
