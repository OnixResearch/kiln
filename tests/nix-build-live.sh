#!/usr/bin/env sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
cargo test -p kiln-adapter-nix --features nix-live --offline
cargo test -p kiln-nix-build --features nix-live --offline
