#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
revision=317d3ff648f01a5ea1572e6126f4dd71bc7afa99
nar_hash='sha256-8NcPYYKMZsOV86rxpQ5Wk61rKbCPBA9ZtbYE3pG1SqA='
source_default_hash=18d7c4ec3985108cde68fd7022f51dd62b69f8e4d9c98c22c19e397301088fed
source_contracts_hash=46ebe026e1b69bb6bee7e24947bd5d18be307e4d977100dd6f8f8fc696d1a15e
previous_default_hash=6a031f3a92ae55bc79eb2417ccb7289c2aabdeda639570563d4cdb294bd316db
previous_contracts_hash=46ebe026e1b69bb6bee7e24947bd5d18be307e4d977100dd6f8f8fc696d1a15e

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
