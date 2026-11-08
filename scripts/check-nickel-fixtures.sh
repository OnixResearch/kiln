#!/usr/bin/env bash
# Check every Nickel fixture without the Nix build sandbox: negatives must be
# rejected and positives must export.
# Requires `nickel` on PATH, normally supplied by `nix develop`.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

negative_count=0
for fixture in config/fixtures/negative-*.ncl; do
  negative_count=$((negative_count + 1))
  if nickel export "$fixture" >/dev/null 2>&1; then
    printf 'negative Nickel fixture passed: %s\n' "$fixture" >&2
    exit 1
  fi
done

positive_count=0
for fixture in config/fixtures/*positive*.ncl; do
  positive_count=$((positive_count + 1))
  if ! nickel export "$fixture" >/dev/null; then
    printf 'positive Nickel fixture failed: %s\n' "$fixture" >&2
    exit 1
  fi
done

nickel export config/hosted-pipeline-example.ncl >/dev/null
printf 'Nickel fixture gate passed (%s positive, %s negative)\n' "$positive_count" "$negative_count"
