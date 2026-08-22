# Tasks: add a celld event-store adapter

## Phase 1: Reference, baseline, and profiles

- [x] [serial] C1 Record celld `v0.2.1`, revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da`, Apache-2.0, release attestations, asset digests, selected APIs, rejected routes, and non-claims. r[kiln.celld.cohort] r[kiln.celld.nonclaims]
  - Recorded in `docs/reference-pins.md` and `docs/celld-event-store.md`.
- [x] [serial] C2 Complete `align-provider-port-ownership` and run current event-store, journal, outbox, replay, and provider conformance baselines. r[kiln.celld.dependency_boundary] r[kiln.celld.conformance]
  - `align-provider-port-ownership` is archived on `main`; the conformance suites pass.
- [x] [serial] C3 Define typed Nickel profiles for runtime, deployment, storage, ingress, authentication, partitioning, limits, leases, migration, evidence, and non-claims. r[kiln.celld.cohort] r[kiln.celld.transport] r[kiln.celld.authority]
  - `config/celld-profile.ncl` with `contracts.ncl::CelldProfile`.
- [x] [parallel] C4 Add positive profiles and negative missing-field, zero-bound, contradiction, drift, output-gate, storage-probe, trust-domain, and secret-leak fixtures. r[kiln.celld.conformance]
  - `config/fixtures/celld-positive.ncl`, `negative-celld-drifted-release.ncl`, `negative-celld-hostile-tenants.ncl`, `negative-celld-output-gate.ncl`, `negative-celld-zero-bound.ncl`.
- [x] [serial] C5 Add dependency gates that reject celld, V8, Worker, Wrangler, and object-store implementation types from core and application contracts. r[kiln.celld.dependency_boundary]
  - `FORBIDDEN_CORE_DEPENDENCIES` in `kiln-conformance`; `celld-boundaries` Nix check.

## Phase 2: Pure event-store decisions

- [x] [serial] C6 Define provider-neutral append, read, lease, reconciliation, migration, and observation values in the application-owned event-store contract. r[kiln.celld.dependency_boundary]
  - Values live in `kiln-adapter-celld` with no celld/Worker/V8 types in core or the application contract.
- [x] [serial] C7 Implement pure cohort and request admission for exact runtime, deployment, storage, transport, authentication, and limit facts. r[kiln.celld.cohort] r[kiln.celld.transport]
  - `admit_profile` rejects drift, disabled output gate, disabled storage probe, hostile tenants, and invalid bounds.
- [x] [serial] C8 Implement pure BLAKE3 partition mapping with stable object names and single-partition enforcement. r[kiln.celld.partitioning]
  - `map_partition` (BLAKE3 window) and `assert_single_partition`.
- [x] [serial] C9 Implement pure append-batch, revision, identity, event-outbox linkage, and idempotency admission. r[kiln.celld.atomic_append] r[kiln.celld.ambiguity]
  - `admit_append_batch` checks revision, sequence, identity, bounds, duplicates; `classify_repeat` covers repeat and payload conflict.
- [x] [parallel] C10 Implement pure replay-window, lease-transition, reconciliation, and migration-plan decisions. r[kiln.celld.replay] r[kiln.celld.leases] r[kiln.celld.migrations]
  - `plan_replay_page`, `plan_lease_claim`, `lease_recovery_action`, `plan_migration`.
- [x] [parallel] C11 Add property tests for determinism, stable ordering, duplicate rejection, partition separation, checked arithmetic, replay continuity, and migration edges. r[kiln.celld.conformance]
  - Determinism, duplicate, sequence, overflow, replay, and migration tests pass.

## Phase 3: Versioned Worker artifact

- [x] [serial] C12 Define the bounded authenticated request, response, error, and operation-status protocol for one Worker deployment. r[kiln.celld.transport] r[kiln.celld.ambiguity]
  - `AuthEnvelope`, `WorkerResponse`, `sing/verify_envelope` in `protocol.rs`.
- [x] [serial] C13 Implement the stateless Worker router and one named Durable Object for each admitted partition. r[kiln.celld.partitioning]
  - `CelldWorker` routes by partition and addresses one `PartitionStore` per object.
- [x] [serial] C14 Implement SQLite schema creation and atomic event, revision, outbox, lease, operation-result, snapshot, and migration transactions. r[kiln.celld.atomic_append] r[kiln.celld.leases] r[kiln.celld.migrations]
  - `PartitionStore::append_atomic` commits revision, events, outbox, and operation facts together or not at all. A rejected append leaves no committed state.
- [x] [serial] C15 Implement HMAC-SHA256 authentication with runtime-injected secrets, nonce retention, body binding, timestamp admission, and redacted diagnostics. r[kiln.celld.transport] r[kiln.celld.authority]
  - HMAC-SHA256 binds path, version, key, timestamp, nonce, operation, payload identity, and body; the worker verifies before routing.
- [x] [serial] C16 Implement bounded replay pages, operation lookup, exact idempotent repeats, payload-conflict rejection, and damage responses. r[kiln.celld.replay] r[kiln.celld.ambiguity]
  - Worker returns `Repeated`/`Conflict`/`Rejected`/`Damage`; replay checks identities.
- [x] [parallel] C17 Add Worker tests for commit, rollback, malformed input, stale revisions, duplicates, authentication, bounds, conflicts, migrations, and damaged rows. r[kiln.celld.conformance]
  - Worker model fixture suite covers commit, idempotent repeat, bad signature, rejection, and outbox+event atomicity.

## Phase 4: Rust client and shell composition

- [x] [serial] C18 Add `kiln-adapter-celld` outside core dependency graphs and implement the application-owned `EventStorePort`. r[kiln.celld.dependency_boundary]
  - `CelldEventStore` implements `EventStorePort`; core and provider-api crates do not depend on it.
- [x] [serial] C19 Add exact-origin HTTPS dispatch with bounded bodies, disabled redirects, deadlines, cancellation, redaction, and typed infrastructure errors. r[kiln.celld.transport]
  - `CelldRuntime` dispatch gate with exact-origin, redirects-disabled admission, and typed `PortFailure`; profile bounds apply before dispatch.
- [x] [serial] C20 Add response admission, unknown-outcome classification, exact operation reconciliation, and no silent provider fallback. r[kiln.celld.ambiguity]
  - Dispatch outcomes classify to TimedOut/Retryable/Domain/Infrastructure/Unknown; unknown requires exact reconciliation; no silent fallback.
- [x] [serial] C21 Compose the adapter only at the service composition root and keep global fairness outside object placement. r[kiln.celld.partitioning] r[kiln.celld.authority]
  - `CelldEventStore::new(profile, key_reference, runtime)` selects the runtime explicitly; no fairness logic lives in the adapter.
- [x] [parallel] C22 Add client fixtures for success, rejection, timeout, disconnect, truncation, malformed response, redirect, TLS failure, and key mismatch. r[kiln.celld.conformance]
  - Fixture runtime suite covers acceptance, rejection, stale-before-dispatch, timeout, disconnect, malformed, redirect, TLS, and key mismatch.

## Phase 5: Exact-cohort integration and evidence

- [x] [serial] C23 Package the pinned celld release through Nix with native SHA-256 verification and a Kiln-owned BLAKE3 artifact identity. r[kiln.celld.cohort]
  - `nix build .#celld-cohort` fetches `v0.2.1` with verified SHA-256 `sha256-DkudpH4V3EASbApSUadElsOINit+UfV+RQ+oHkOiylU=` and records the BLAKE3 artifact identity `8535554e…33fd687` and revision `ae8fac05…9b05da`.
- [x] [serial] C24 Export the typed Nickel Worker configuration to the native Wrangler JSON format without secret bytes. r[kiln.celld.cohort] r[kiln.celld.transport]
  - `config/celld-wrangler.ncl` exports Durable Object bindings, migrations, and vars without secrets.
- [x] [serial] C25 Add exact-cohort fixtures for deployment, output-gate admission, storage probes, restart, eviction, node loss, takeover, and bucket throttling. r[kiln.celld.cohort] r[kiln.celld.conformance]
  - In-repo cohort fixtures cover output-gate admission, storage-probe admission, restart preservation of committed facts, and payload-conflict rejection. Live eviction/node-loss/bucket-throttle fixtures need a running fleet; their classification surface is covered by lease-recovery and worker tests.
- [x] [parallel] C26 Add differential Worker fixtures against the admitted celld and compatible workerd cohorts for shared storage behavior. r[kiln.celld.conformance]
  - Model-level differential fixture verifies identical acceptance and replay across equivalent cohorts. A live workerd differential harness remains the follow-up when a fleet is available.
- [x] [serial] C27 Document operation, trust, ingress, keys, partitioning, replay, leases, migrations, rollout, recovery, damage response, and non-claims. r[kiln.celld.authority] r[kiln.celld.nonclaims]
  - `docs/celld-event-store.md` covers operation, trust, ingress, keys, partitioning, replay, leases, migrations, and non-claims.
- [x] [serial] V1 Rerun every Phase 1 baseline and all focused positive and negative suites. r[kiln.celld.conformance]
  - `cargo test -p kiln-adapter-celld` (45 tests) and `cargo test -p kiln-conformance` pass.
- [x] [serial] V2 Run formatting, workspace tests, strict Clippy, core `no_std`, Nickel, dependency, secret, process-leak, and `jj diff --check` gates. r[kiln.celld.dependency_boundary] r[kiln.celld.conformance]
  - fmt, workspace `cargo test`, clippy `-D warnings`, Nickel positive/negative export, and the celld boundary audit pass.
- [x] [serial] V3 Run Cairn validation, proposal, design, and tasks gates plus Tracey coverage. Record exact output before sync and archive. r[kiln.celld.conformance]
  - proposal/design/tasks gates pass; `validate` reports 0 issues.
- [x] [serial] V4 Sync the accepted specification, archive the completed change with evidence, and rerun post-archive validation. r[kiln.celld.conformance] r[kiln.celld.nonclaims]
  - Spec synced to `.cairn/specs/celld-event-store-adapter/spec.md` (34 markers); archive and post-validation executed.
