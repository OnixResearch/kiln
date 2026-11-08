#!/usr/bin/env bash
# Check every Nickel fixture without the Nix build sandbox: negatives must be
# rejected and positives must export.
# Requires `nickel` on PATH, normally supplied by `nix develop`.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

for fixture in config/fixtures/negative-*.ncl; do
  if nickel export "$fixture" >/dev/null 2>&1; then
    printf 'negative Nickel fixture passed: %s\n' "$fixture" >&2
    exit 1
  fi
done

for fixture in config/fixtures/*positive*.ncl; do
  nickel export "$fixture" >/dev/null
done

nickel export config/hosted-pipeline-example.ncl >/dev/null
printf 'Nickel fixture gate passed\n'
