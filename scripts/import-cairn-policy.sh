#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
revision=9082e1aece4f04bcbead5eeea087e496bc48a675
nar_hash='sha256-MupXo6ha88a5GtCWyurc6ogIw5N0i7nat2BQ3btglnA='
source_default_hash=6a031f3a92ae55bc79eb2417ccb7289c2aabdeda639570563d4cdb294bd316db
source_contracts_hash=46ebe026e1b69bb6bee7e24947bd5d18be307e4d977100dd6f8f8fc696d1a15e
previous_default_hash=c3b5981d71789b04ff3a7567a84a7b1df29b5703dcb3a65e664143713e77a97e
previous_contracts_hash=bc537dc32ff01fc9004dc032b78ad6fb5855c5c6f0158ecda219d5912b46619c

actual_hash() { b3sum --no-names "$1"; }
require_hash() {
  local observed
  observed=$(actual_hash "$1")
  if [[ "$observed" != "$2" && "$observed" != "$3" ]]; then
    printf 'refusing modified Cairn policy snapshot: %s\n' "$1" >&2
    exit 1
  fi
}
require_hash "$root/cairn-policy/upstream-default.ncl" "$previous_default_hash" "$source_default_hash"
require_hash "$root/cairn-policy/contracts.ncl" "$previous_contracts_hash" "$source_contracts_hash"

metadata=$(nix flake metadata --json "git+ssh://git@github.com/OnixResearch/cairn.git?rev=$revision")
if ! jq -e --arg rev "$revision" --arg nar "$nar_hash" \
  '.locked.rev == $rev and .locked.narHash == $nar' <<<"$metadata" >/dev/null; then
  printf 'refusing Cairn source with unexpected revision or NAR identity\n' >&2
  exit 1
fi
source=$(jq -er '.path' <<<"$metadata")
if [[ "$(actual_hash "$source/cairn-policy/default.ncl")" != "$source_default_hash" ||
      "$(actual_hash "$source/cairn-policy/contracts.ncl")" != "$source_contracts_hash" ]]; then
  printf 'refusing Cairn policy source with unexpected file bytes\n' >&2
  exit 1
fi
install -m 0644 "$source/cairn-policy/default.ncl" "$root/cairn-policy/upstream-default.ncl"
install -m 0644 "$source/cairn-policy/contracts.ncl" "$root/cairn-policy/contracts.ncl"
printf 'imported Cairn policy revision %s (%s)\n' "$revision" "$nar_hash"
