# Verification Evidence

## Live defect reproduction

On 2026-08-28, production shadow operation `65c700f8886f9112db18c661bfc85a5b15005ef2562db1f69e01bb3ba464d809` dispatched one real Lattice workflow. The host exhausted 1,023 unpaced observations in 286 milliseconds and retained the operation as `Unknown`.

The provider completed after the host stopped polling. It published run `b3:abfea36e3697f9eff3a0c1758ca441291409db30b0d940f8c478b482785b0694` with mode `0640` and failure exit code `1`. The report exposed a separate missing-Git wrapper dependency, so it is not cutover evidence.

- report BLAKE3: `58f32f254cc8af1f37f8072db41a2068c196353f8bfe82093604d39dc5687a5b`;
- log BLAKE3: `0f8a28ffcd7e12c46298bf096a84f0b945417aa3b2520145f1ef46354f641e63`.

No production cutover occurred.

## Baseline

Before the core change:

- default `kiln-aspen-host` tests: PASS;
- all-feature `kiln-aspen-host` tests: PASS, including four hosted positive and negative fixtures.

Cargo prints the intentional malformed Aspen fixture diagnostic before each successful run.

## Changed implementation

The host now requires `--poll-interval-ms`. It rejects missing, duplicate, zero, and malformed values. Normal dispatch and `RecoverDispatch` wait only after a non-terminal observation. The existing finite count still bounds every loop.

Verification completed:

- focused delayed-terminal and exhaustion tests: PASS;
- workspace, all-target, all-feature host tests: PASS;
- strict all-target, all-feature Clippy with `-D warnings`: PASS;
- strict Cairn validation before and after implementation: PASS;
- Nix host package check: PASS;
- Nix Octet deny-all check: PASS with zero findings, warnings, or errors.

The first Nix Octet run rejected a six-parameter constructor. The corrected implementation uses `LatticeEffectPortOptions`; no lint allowance was added. A later run rejected system-clock reads in tests and an oversized test file. The final design injects an `ObservationPacer`, uses a deterministic counting adapter, and keeps support code in a narrow module. The official Nix deny-all gate is clean.

Live consumer evidence follows after publication.
