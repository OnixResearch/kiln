#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
revision=007ba5193dd8fec9067a1c0e0f8ef487807af872
nar_hash='sha256-Kgx8JgsC/+26ETbL3lRaxXAYfzv2cmtG/i9OywPOz0c='
source_default_hash=c3b5981d71789b04ff3a7567a84a7b1df29b5703dcb3a65e664143713e77a97e
source_contracts_hash=bc537dc32ff01fc9004dc032b78ad6fb5855c5c6f0158ecda219d5912b46619c
previous_default_hash=97c79749114d24ce963049f525ffe4e783f094a90e222fcc7fdd2487a5c8d5d8
previous_contracts_hash=845a0f4038dfc198bc0a4aa72672b570f9d8f0e64ae821aec6c719736f53188f

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
