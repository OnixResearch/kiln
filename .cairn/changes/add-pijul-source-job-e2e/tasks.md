# Tasks: Pijul Source-to-CI E2E

## Phase 1: Shared fixtures

- [ ] [serial] Extract the pinned Pijul repository, record, and channel helpers into `tests/support/mod.rs`. r[kiln.source.pijul_e2e]
- [ ] [serial] Preserve the existing native conformance tests against the shared helpers. r[kiln.source.pijul_e2e]

## Phase 2: Full composition

- [ ] [serial] Resolve and acquire one exact Pijul channel state through the public adapter port. r[kiln.source.pijul_e2e]
- [ ] [serial] Materialize only admitted snapshot entries and reject unsafe paths or metadata. r[kiln.source.pijul_e2e]
- [ ] [serial] Run a reviewed bounded command over the exact snapshot and assert `.pijul` is absent. r[kiln.source.pijul_e2e.scenario.pass]
- [ ] [serial] Admit the workflow observation and apply StartRun and CompleteAttempt for the pass outcome. r[kiln.source.pijul_e2e.scenario.pass]
- [ ] [serial] Prove a genuine nonzero command produces `DomainFailure` through the same path. r[kiln.source.pijul_e2e.scenario.fail]
- [ ] [serial] Cross-check the exact state and repository through the `SourceResolution` mapping. r[kiln.source.pijul_e2e]

## Phase 3: Verification

- [ ] [serial] Run both native Pijul test targets with the pinned `KILN_PIJUL_EXE`. r[kiln.source.pijul_e2e]
- [ ] [serial] Update the Nix `pijul-native-fixtures` check to run both test targets. r[kiln.source.pijul_e2e]
- [ ] [serial] Run focused tests, Clippy, Cairn validation, and the Nix check. r[kiln.source.pijul_e2e]
- [ ] [serial] Record evidence and non-claims. r[kiln.source.pijul_e2e]
