# Change-local acceptance

a[align-cairn-implementation-ready.imports] `vendor/cairn` and `vendor/cairn-verifier` equal `git archive 9082e1aece4f04bcbead5eeea087e496bc48a675` for their recorded paths apart from the importer's recorded normalizations, their source records and importer pins agree, and `vendor/cairn-kiln-core` still verifies at `5d99f4e9`.

a[align-cairn-implementation-ready.gate-wire] The gate executable profile stays at its reviewed `15f00875` cohort, and `cairn-gate-cohort` refuses the new import if its producer fixtures or Nickel contract differ from the frozen `15f00875` copies.

a[align-cairn-implementation-ready.policy] The `kiln-default` policy snapshot is re-imported at `9082e1ae` with its importer, re-exported by Cairn, and loaded by a `9082e1ae` `cairn`; the old export is refused by that `cairn`; and the agent-completion fixtures carry the new policy hash, so release readiness reports no policy drift.

a[align-cairn-implementation-ready.verification] Workspace fmt, clippy with and without all features, workspace tests, Nickel fixtures, `cairn validate`, traceability and the flake checks and packages that build the imports pass, and `Cargo.lock` changes only the `cairn-gate-contract` version.
