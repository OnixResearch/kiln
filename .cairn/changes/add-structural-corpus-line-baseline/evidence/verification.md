# Verification evidence

## Corpus

OmniDiff at `7d27fe8b27236a43c44b24e81da4c3fbb0cd8408`, fetched outside the
repository.

## Result

- 727 fixtures in languages Kiln parses.
- 611 parsed structurally.
- 145 parsed fixtures have zero mismatched old/new lines (23.7%).
- 116 fall back: 99 parse errors, 16 too large, 1 too many nodes.
- Parsed-fixture p99 is 43 ms (bash); the slowest p50 is 1.05 ms (rust).
  Timing is printed to stderr and is not part of the baseline gate.
- Re-running against `baseline.csv` reports no regressions and byte-identical
  output.

## Non-claims

Line agreement is not node-mapping fidelity. Moves and latency are not scored.
A passing baseline does not prove structural diff correctness, grammar memory
safety, review readiness or release eligibility.
