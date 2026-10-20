#!/usr/bin/env sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

# Make the real discovery producer available without relying on a prior check.
cohort_drv=$(nix eval --raw "path:$root#checks.x86_64-linux.nix-eval-jobs-cohort.drvPath")
nix-store -q --references "$cohort_drv" |
  grep -E 'nix-eval-jobs|cohort' |
  while IFS= read -r reference; do
    case "$reference" in
      *.drv) nix-store --realise "$reference" >/dev/null ;;
    esac
  done

# Materialize the fixture flake as an immutable store source.
KILN_NIX_JOB_SOURCE=$(nix store add-path \
  "$root/fixtures/nix-eval-jobs/cohort/a0cd02231c58974a6b5aaa3712069b071047162e/jobset" \
  --extra-experimental-features nix-command)
export KILN_NIX_JOB_SOURCE

cargo test -p kiln-adapter-nix --features nix-live --offline
cargo test -p kiln-nix-build --features nix-live --offline
