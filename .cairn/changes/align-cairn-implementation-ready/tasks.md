## Phase 1: Imports

- [ ] [serial] Re-import `vendor/cairn` and `vendor/cairn-verifier` at Cairn `9082e1ae` through `scripts/import-pinned-source.sh`, updating the source records and importer pins together; compare each tree with `git archive` and confirm `vendor/cairn-kiln-core` still verifies at `5d99f4e9`. a[align-cairn-implementation-ready.imports]
- [ ] [serial] Regenerate `Cargo.lock` with Cargo and require `cairn-gate-cohort` to bind the new import to the frozen `15f00875` gate fixtures and contract. a[align-cairn-implementation-ready.gate-wire]
- [ ] [serial] Re-import the Cairn policy snapshot at `9082e1ae` with `scripts/import-cairn-policy.sh`, re-export `cairn-policy/generated/cairn-policy.json`, rebind the agent-completion fixtures to its hash, and confirm the `9082e1ae` `cairn` loads it, refuses the previous export and passes release readiness. a[align-cairn-implementation-ready.policy]

## Phase 2: Verification

- [ ] [serial] Update the reference pins, Cairn gate guide, policy provenance, license map, README and changelog; run fmt, both clippy variants, workspace tests, Nickel fixtures, `cairn validate`, traceability and the flake checks and packages that build the imports, and record the evidence. a[align-cairn-implementation-ready.verification]
