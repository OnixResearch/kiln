## Phase 1: Cohort and profile

- [x] [serial] Pin the exact SQLite-free Radicle crate cohort and SecretSpec CLI versions, source revisions, licenses, and supported platform. r[kiln.radicle.status_signer] r[kiln.radicle.status_allowlist]
- [x] [depends:status-cohort-pin] Add a typed Nickel status profile with SecretSpec declaration fields, RID and type allowlists, single-writer policy, named bounds, and non-claims. r[kiln.radicle.status_allowlist] r[kiln.radicle.status_signer]
- [x] [parallel] Add positive profile fixtures and negative missing-field, zero-bound, unknown-provider, wrong-type, and foreign-RID fixtures. r[kiln.radicle.status_conformance]

## Phase 2: Pure decisions

- [x] [depends:status-cohort-pin] Add pure status plan admission, single-writer head comparison, idempotency decisions, and outcome classification. r[kiln.radicle.shell_status] r[kiln.radicle.status_allowlist]
- [x] [parallel] Add positive and negative core tests for substitution, stale heads, replayed keys, and unknown outcomes. r[kiln.radicle.shell_status] r[kiln.radicle.status_conformance]

## Phase 3: Concrete shell

- [x] [depends:status-profile] Implement the SecretSpec resolver shell with fail-closed denial and redaction. r[kiln.radicle.status_signer]
- [x] [depends:status-signer] Implement the bounded `radicle-cob` status writer with single-writer preflight and reconciliation. r[kiln.radicle.shell_status] r[kiln.radicle.status_allowlist]

## Phase 4: Fixtures and evidence

- [x] [depends:status-shell] Add native positive and negative repository fixtures created by the pinned cohort, including repeat writes, foreign heads, and missing declarations. r[kiln.radicle.status_conformance]
- [x] [parallel] Add runtime-independence checks that keep SecretSpec and Radicle crates outside core and standalone builds. r[kiln.radicle.status_signer]
- [x] [parallel] Document setup, SecretSpec configuration, single-writer policy, bounds, diagnostics, and non-claims. r[kiln.radicle.status_signer] r[kiln.radicle.status_allowlist]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and focused Nix gates. r[kiln.radicle.status_conformance]
- [x] [serial] Record evidence, sync accepted specs, archive the change, and rerun validation. r[kiln.radicle.status_conformance]
