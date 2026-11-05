# Structural review corpus harness

Out-of-tree measurement for `kiln_highlight::structural_diff` against
OmniDiff's hand-painted fixtures. It is a measurement tool, not a release gate.

## Corpus

Pinned upstream revision: `7d27fe8b27236a43c44b24e81da4c3fbb0cd8408`
(`ivankovic/omnidiff`). The corpus is fetched outside this repository; do not
vendor its 242 MiB tree.

```console
git init /tmp/omnidiff
git -C /tmp/omnidiff remote add origin https://github.com/ivankovic/omnidiff
git -C /tmp/omnidiff fetch --depth 1 origin 7d27fe8b27236a43c44b24e81da4c3fbb0cd8408
git -C /tmp/omnidiff checkout FETCH_HEAD
```

## Run

```console
cargo run --release --manifest-path checks/structural-corpus/Cargo.toml -- \
  /tmp/omnidiff/src/test/data/diffs checks/structural-corpus/baseline.csv
```

The first argument is the corpus root; the optional second argument is the
per-fixture baseline. A fixture that starts falling back or whose old/new
mismatched-line count grows fails the run. The CSV on stdout is the current
per-fixture result.

## Current baseline

At the pinned corpus revision:

- 727 fixtures in languages Kiln parses.
- 611 parsed structurally; 116 fall back (99 parse errors, 16 too large, 1 too
  many nodes).
- 145 of 611 parsed fixtures (23.7%) have zero mismatched old/new lines.
- No fixture lands between zero and 1% mismatch; the at-most-1% share equals
  the zero-mismatch share.
- Parsed-fixture p99 latency is 41 ms (rust); the slowest p50 is 1.10 ms
  (rust). Timing is printed to stderr, not written to the baseline CSV.

Per-language table from the pinned run:

```text
language,fixtures,parsed,fallback,zero_mismatch,at_most_1pct,p50_us,p90_us,p99_us
bash,55,49,6,12,12,295,29857,40351
c,89,37,52,9,9,364,6514,11951
css,38,30,8,7,7,276,1496,5556
go,79,77,2,21,21,602,10199,18958
html,57,47,10,3,3,169,2634,5093
javascript,53,33,20,11,11,141,1769,12105
json,52,48,4,13,13,164,4535,10251
python,71,70,1,16,16,474,6086,15026
rust,112,105,7,29,29,1095,13918,41440
tsx,41,40,1,9,9,232,3807,4244
typescript,43,38,5,4,4,140,4404,8301
yaml,37,37,0,11,11,191,1982,9560
```

## Boundaries

- Line agreement is projected from OmniDiff's `Full` text mappings and Kiln's
  novel-token rows. It is not node-mapping fidelity.
- Moves are not scored. Timing is reported but not gated.
- A passing baseline does not prove structural diff correctness, grammar memory
  safety, review readiness or release eligibility.
