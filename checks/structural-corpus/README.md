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
- Parsed-fixture p99 latency is 43 ms (bash); the slowest p50 is 1.05 ms
  (rust). Timing is printed to stderr, not written to the baseline CSV.

## Boundaries

- Line agreement is projected from OmniDiff's `Full` text mappings and Kiln's
  novel-token rows. It is not node-mapping fidelity.
- Moves are not scored. Timing is reported but not gated.
- A passing baseline does not prove structural diff correctness, grammar memory
  safety, review readiness or release eligibility.
