# Change-local acceptance

a[align-cairn-implementation-ready.imports] `vendor/cairn` and `vendor/cairn-verifier` equal `git archive 317d3ff648f01a5ea1572e6126f4dd71bc7afa99` for their recorded paths apart from the importer's recorded normalizations, their source records and importer pins agree, and `vendor/cairn-kiln-core` still verifies at `5d99f4e9`.

a[align-cairn-implementation-ready.gate-wire] The gate executable profile stays at its reviewed `15f00875` cohort, and `cairn-gate-cohort` refuses the new import if its producer fixtures or Nickel contract differ from the frozen `15f00875` copies.

a[align-cairn-implementation-ready.policy] The `kiln-default` policy snapshot is re-imported at `317d3ff6` with its importer, re-exported by Cairn, and loaded by a `317d3ff6` `cairn`; the generated catalog files `readiness` and `drain-plan-readiness` under the `readiness` class; the old export is refused by that `cairn`; and the agent-completion fixtures carry the new policy hash, so release readiness reports no policy drift.

a[align-cairn-implementation-ready.verification] Workspace fmt, clippy with and without all features, workspace tests, Nickel fixtures, `cairn validate`, traceability, release readiness and the flake checks and packages that build the imports pass, a real `cairn implementation-ready` output is admitted through `kiln-cairn-intake`, and `Cargo.lock` changes only the `cairn-gate-contract` version.
