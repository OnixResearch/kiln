# Tasks: add a celld event-store adapter

## Phase 1: Reference, baseline, and profiles

- [ ] [serial] C1 Record celld `v0.2.1`, revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da`, Apache-2.0, release attestations, asset digests, selected APIs, rejected routes, and non-claims. r[kiln.celld.cohort] r[kiln.celld.nonclaims]
- [ ] [serial] C2 Complete `align-provider-port-ownership` and run current event-store, journal, outbox, replay, and provider conformance baselines. r[kiln.celld.dependency_boundary] r[kiln.celld.conformance]
- [ ] [serial] C3 Define typed Nickel profiles for runtime, deployment, storage, ingress, authentication, partitioning, limits, leases, migration, evidence, and non-claims. r[kiln.celld.cohort] r[kiln.celld.transport] r[kiln.celld.authority]
- [ ] [parallel] C4 Add positive profiles and negative missing-field, zero-bound, contradiction, drift, output-gate, storage-probe, trust-domain, and secret-leak fixtures. r[kiln.celld.conformance]
- [ ] [serial] C5 Add dependency gates that reject celld, V8, Worker, Wrangler, and object-store implementation types from core and application contracts. r[kiln.celld.dependency_boundary]

## Phase 2: Pure event-store decisions

- [ ] [serial] C6 Define provider-neutral append, read, lease, reconciliation, migration, and observation values in the application-owned event-store contract. r[kiln.celld.dependency_boundary]
- [ ] [serial] C7 Implement pure cohort and request admission for exact runtime, deployment, storage, transport, authentication, and limit facts. r[kiln.celld.cohort] r[kiln.celld.transport]
- [ ] [serial] C8 Implement pure BLAKE3 partition mapping with stable object names and single-partition enforcement. r[kiln.celld.partitioning]
- [ ] [serial] C9 Implement pure append-batch, revision, identity, event-outbox linkage, and idempotency admission. r[kiln.celld.atomic_append] r[kiln.celld.ambiguity]
- [ ] [parallel] C10 Implement pure replay-window, lease-transition, reconciliation, and migration-plan decisions. r[kiln.celld.replay] r[kiln.celld.leases] r[kiln.celld.migrations]
- [ ] [parallel] C11 Add property tests for determinism, stable ordering, duplicate rejection, partition separation, checked arithmetic, replay continuity, and migration edges. r[kiln.celld.conformance]

## Phase 3: Versioned Worker artifact

- [ ] [serial] C12 Define the bounded authenticated request, response, error, and operation-status protocol for one Worker deployment. r[kiln.celld.transport] r[kiln.celld.ambiguity]
- [ ] [serial] C13 Implement the stateless Worker router and one named Durable Object for each admitted partition. r[kiln.celld.partitioning]
- [ ] [serial] C14 Implement SQLite schema creation and atomic event, revision, outbox, lease, operation-result, snapshot, and migration transactions. r[kiln.celld.atomic_append] r[kiln.celld.leases] r[kiln.celld.migrations]
- [ ] [serial] C15 Implement HMAC-SHA256 authentication with runtime-injected secrets, nonce retention, body binding, timestamp admission, and redacted diagnostics. r[kiln.celld.transport] r[kiln.celld.authority]
- [ ] [serial] C16 Implement bounded replay pages, operation lookup, exact idempotent repeats, payload-conflict rejection, and damage responses. r[kiln.celld.replay] r[kiln.celld.ambiguity]
- [ ] [parallel] C17 Add Worker tests for commit, rollback, malformed input, stale revisions, duplicates, authentication, bounds, conflicts, migrations, and damaged rows. r[kiln.celld.conformance]

## Phase 4: Rust client and shell composition

- [ ] [serial] C18 Add `kiln-adapter-celld` outside core dependency graphs and implement the application-owned `EventStorePort`. r[kiln.celld.dependency_boundary]
- [ ] [serial] C19 Add exact-origin HTTPS dispatch with bounded bodies, disabled redirects, deadlines, cancellation, redaction, and typed infrastructure errors. r[kiln.celld.transport]
- [ ] [serial] C20 Add response admission, unknown-outcome classification, exact operation reconciliation, and no silent provider fallback. r[kiln.celld.ambiguity]
- [ ] [serial] C21 Compose the adapter only at the service composition root and keep global fairness outside object placement. r[kiln.celld.partitioning] r[kiln.celld.authority]
- [ ] [parallel] C22 Add client fixtures for success, rejection, timeout, disconnect, truncation, malformed response, redirect, TLS failure, and key mismatch. r[kiln.celld.conformance]

## Phase 5: Exact-cohort integration and evidence

- [ ] [serial] C23 Package the pinned celld release through Nix with native SHA-256 verification and a Kiln-owned BLAKE3 artifact identity. r[kiln.celld.cohort]
- [ ] [serial] C24 Export the typed Nickel Worker configuration to the native Wrangler JSON format without secret bytes. r[kiln.celld.cohort] r[kiln.celld.transport]
- [ ] [serial] C25 Add exact-cohort fixtures for deployment, output-gate admission, storage probes, restart, eviction, node loss, takeover, and bucket throttling. r[kiln.celld.cohort] r[kiln.celld.conformance]
- [ ] [parallel] C26 Add differential Worker fixtures against the admitted celld and compatible workerd cohorts for shared storage behavior. r[kiln.celld.conformance]
- [ ] [serial] C27 Document operation, trust, ingress, keys, partitioning, replay, leases, migrations, rollout, recovery, damage response, and non-claims. r[kiln.celld.authority] r[kiln.celld.nonclaims]
- [ ] [serial] V1 Rerun every Phase 1 baseline and all focused positive and negative suites. r[kiln.celld.conformance]
- [ ] [serial] V2 Run formatting, workspace tests, strict Clippy, core `no_std`, Nickel, dependency, secret, process-leak, and `jj diff --check` gates. r[kiln.celld.dependency_boundary] r[kiln.celld.conformance]
- [ ] [serial] V3 Run Cairn validation, proposal, design, and tasks gates plus Tracey coverage. Record exact output before sync and archive. r[kiln.celld.conformance]
- [ ] [serial] V4 Sync the accepted specification, archive the completed change with evidence, and rerun post-archive validation. r[kiln.celld.conformance] r[kiln.celld.nonclaims]
