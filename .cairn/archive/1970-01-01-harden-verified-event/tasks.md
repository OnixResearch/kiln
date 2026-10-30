## Phase 1: Seal the type

- [x] [serial] Make every `VerifiedEvent` field private, add read-only getters, and remove the `Serialize`/`Deserialize` derives. r[kiln.github.event_proof]
- [x] [serial] Add the two `compile_fail` doctests (struct literal, JSON deserialization) and a positive getter example; replace the one module test that serialized the event. r[kiln.github.event_proof.scenario.forgery]

## Phase 2: Consumers and audit

- [x] [serial] Update `kiln-hosted`, `kiln-web-service`, `service.rs`, `replay.rs` and module tests to read through getters without changing selection or identities. r[kiln.github.event_proof]
- [x] [serial] Record the `Admitted*` audit in the design with one line per type and no changes to those types. r[kiln.github.event_proof.audit] r[kiln.github.event_proof.audit.scenario.names]

## Phase 3: Verification

- [x] [serial] Run doc tests and the adapter, hosted and web-service suites; run workspace clippy and fmt; run the affected flake checks; record evidence. r[kiln.github.event_proof] r[kiln.github.event_proof.scenario.forgery]
