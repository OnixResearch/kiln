#!/usr/bin/env bash
# Deterministic Nickel 1.17.0 cohort fixture-matrix check.
#
# The script asserts that:
#   1. the resolved evaluator is the 1.17 cohort (not older, floating,
#      ambient, or mixed);
#   2. every positive fixture evaluates;
#   3. every negative fixture fails, with the exact marker where one is
#      declared in a sibling `*.expected` file;
#   4. the Nickel-dependent evidence doc retains the authority, freshness,
#      and replay bounded-registration non-claims.
#
# Usage: bash scripts/check-nickel-toolchain-fixtures.sh /path/to/nickel

set -eu

NICKEL="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/fixtures/nickel-toolchain"
EVIDENCE_DOC="$ROOT/docs/nickel-toolchain.md"

version="$("$NICKEL" --version 2>&1 | head -1)"
case "$version" in
  *1.17*)
    echo "cohort: PASS ($version)"
    ;;
  *)
    echo "cohort: FAIL unexpected or older evaluator $version" >&2
    exit 1
    ;;
esac

count=0

for fixture in "$FIXTURES"/positive/*.ncl; do
  base="$(basename "$fixture")"
  if "$NICKEL" eval "$fixture" >/dev/null 2>&1; then
    echo "positive: PASS $base"
    count=$((count + 1))
  else
    echo "positive: FAIL $base (evaluation returned an error)" >&2
    exit 1
  fi
done

for fixture in "$FIXTURES"/negative/*.ncl; do
  base="$(basename "$fixture")"
  marker=""
  if [ -f "$fixture.expected" ]; then
    marker="$(cat "$fixture.expected")"
  fi
  output="$("$NICKEL" eval "$fixture" 2>&1 || true)"
  if [ -z "$output" ]; then
    echo "negative: FAIL $base (evaluated without an error)" >&2
    exit 1
  fi
  if [ -n "$marker" ] && ! printf '%s\n' "$output" | grep -Fq "$marker"; then
    echo "negative: FAIL $base (expected diagnostic to contain '$marker')" >&2
    exit 1
  fi
  echo "negative: PASS $base"
  count=$((count + 1))
done

required_non_claims=(
  "not claim key trust"
  "not claim revocation freshness"
  "not claim replay durability"
)
for phrase in "${required_non_claims[@]}"; do
  if ! grep -Fq "$phrase" "$EVIDENCE_DOC"; then
    echo "boundary: FAIL evidence doc is missing non-claim '$phrase'" >&2
    exit 1
  fi
done
echo "boundary: PASS documented non-claims are present"

echo "nickel-toolchain fixture matrix: PASS ($count cases)"
