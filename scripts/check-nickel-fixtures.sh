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

for fixture in config/fixtures/*-negative.ncl; do
  negative_count=$((negative_count + 1))
  if nickel export "$fixture" >/dev/null 2>&1; then
    printf 'negative Nickel fixture passed: %s\n' "$fixture" >&2
    exit 1
  fi
done

for fixture in deployment/fixtures/negative-*.ncl; do
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

# deployment/fixtures/module-positive.ncl imports a generated onix.ncl that is
# not part of the source tree, so only the source-complete positive is checked.
for fixture in deployment/fixtures/positive.ncl; do
  positive_count=$((positive_count + 1))
  if ! nickel export "$fixture" >/dev/null; then
    printf 'positive Nickel fixture failed: %s\n' "$fixture" >&2
    exit 1
  fi
done

positive_count=$((positive_count + 1))
if ! nickel export config/hosted-pipeline-example.ncl >/dev/null; then
  printf 'positive Nickel fixture failed: config/hosted-pipeline-example.ncl\n' >&2
  exit 1
fi

# The Rust projection admission test reads this committed export.
projection_export=$(mktemp)
load_export=$(mktemp)
cases_export=$(mktemp)
corpus_export=$(mktemp)
build_export=$(mktemp)
trap 'rm -f "$projection_export" "$load_export" "$cases_export" "$corpus_export" "$build_export"' EXIT
nickel export --format json config/fixtures/source-projection-positive.ncl >"$projection_export"
if ! cmp -s "$projection_export" fixtures/source-projection/definition.json; then
  printf 'stale export: regenerate fixtures/source-projection/definition.json from config/fixtures/source-projection-positive.ncl\n' >&2
  exit 1
fi

# The review-queue wait fixture and its test read this committed export.
nickel export --format json config/fixtures/review-queue-wait-positive.ncl >"$load_export"
if ! cmp -s "$load_export" fixtures/review-queue-wait/load.json; then
  printf 'stale export: regenerate fixtures/review-queue-wait/load.json from config/fixtures/review-queue-wait-positive.ncl\n' >&2
  exit 1
fi

# The plan-review receipt and its test read this committed export.
nickel export --format json config/fixtures/plan-review-positive.ncl >"$cases_export"
if ! cmp -s "$cases_export" fixtures/plan-review/cases.json; then
  printf 'stale export: regenerate fixtures/plan-review/cases.json from config/fixtures/plan-review-positive.ncl\n' >&2
  exit 1
fi

# The projected-workspace local baseline and its test read this committed export.
nickel export --format json config/fixtures/workspace-corpus-positive.ncl >"$corpus_export"
if ! cmp -s "$corpus_export" fixtures/workspace-corpus/corpus.json; then
  printf 'stale export: regenerate fixtures/workspace-corpus/corpus.json from config/fixtures/workspace-corpus-positive.ncl\n' >&2
  exit 1
fi

# The build-input qualification harness and its tests read this committed export.
nickel export --format json config/fixtures/build-input-corpus-positive.ncl >"$build_export"
if ! cmp -s "$build_export" fixtures/build-inputs/corpus.json; then
  printf 'stale export: regenerate fixtures/build-inputs/corpus.json from config/fixtures/build-input-corpus-positive.ncl\n' >&2
  exit 1
fi

for profile in config/*.ncl deployment/*.ncl; do
  if ! nickel typecheck "$profile" >/dev/null; then
    printf 'Nickel typecheck failed: %s\n' "$profile" >&2
    exit 1
  fi
done

if ! nickel format --check $(find config deployment -name '*.ncl') >/dev/null; then
  printf 'Nickel formatting check failed\n' >&2
  exit 1
fi
printf 'Nickel fixture gate passed (%s positive, %s negative)\n' "$positive_count" "$negative_count"
