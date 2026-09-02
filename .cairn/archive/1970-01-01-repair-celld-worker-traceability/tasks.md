# Tasks

## Phase 1: Honest inventory and lifecycle gates

- [x] [serial] Map each of the 22 accepted Celld scenario IDs to actual maintained behavior or an exact gap; record the three stale implementation links without rewriting accepted obligations. r[kiln.celld.local_worker_evidence]
- [x] [serial] Inspect the pinned v0.2.1 archive, Nix runtime/bundler/SQLite/local S3 inputs, cohort executable and real Worker API; gate this proposal, design and tasks before implementing. r[kiln.celld.local_worker_evidence]

## Phase 2: Real local behavior

- [x] [serial] Implement the missing Kiln-owned Worker entry using authenticated bounded requests and real per-object SQLite atomic journal, outbox, operation and schema storage; reject incompatible routes and reject precommit faults without partial state. r[kiln.celld.local_worker_evidence]
- [x] [serial] Correct the Rust adapter's response-loss classification and fabricated empty load; preserve single-writer, core isolation and conditional retry authority. r[kiln.celld.local_worker_evidence]
- [x] [parallel] Add meaningful positive/negative Worker/SQLite and Rust native tests; link only actually exercised accepted scenario identities, remove three dangling marker identities, and document gaps where a pinned live fleet is not exercised. r[kiln.celld.local_worker_evidence]
- [x] [parallel] Add authenticated conditional Worker lease and schema-migration routes, real SQLite persisted facts, Rust control admission, and positive/negative local tests without routing provider effects. r[kiln.celld.local_worker_evidence]
- [x] [serial] Build and run the self-cleaning pinned v0.2.1 SeaweedFS cohort; measure selected and observed artifact, Worker deployment, storage, ingress, gate and probe; cross-check the HTTPS Worker cohort response; verify append/load, lease/schema restart, mixed-cohort and disabled-gate denial before mutation. r[kiln.celld.local_worker_evidence]

## Phase 3: Qualification

- [x] [serial] Run runnable local Worker and Rust paths, affected workspace checks, Cairn strict validation and all three change gates; rerun global traceability only after substantive changes and record exact counts. r[kiln.celld.local_worker_evidence]
- [x] [serial] Update Celld docs and evidence with actual tests, source and tool limitations. Sync/archive this package and the active status adoption only if accepted obligations obtain legitimate global coverage. r[kiln.celld.local_worker_evidence]

## Local evidence

- `node --test tests/celld-worker.test.mjs`: 16/16 signed real-SQLite Worker checks passed, including forced precommit rollback and persisted lease/schema facts.
- `cargo test --workspace --features kiln-adapter-radicle/status-native-fixtures --offline`: full workspace passed, including Celld adapter 54/54, native status 12/12, patch 6/6 and vendored shared-store 4/4; `cargo clippy --workspace --all-targets --features kiln-adapter-radicle/status-native-fixtures --offline -- -D warnings` and `cargo fmt --all -- --check` passed.
- `tests/celld-live-cohort.sh` rebuilt pinned Celld v0.2.1 and ran signed HTTPS append/lost-response reconciliation, real schema migration and lease persistence across node restart, incompatible schema and disabled-gate predispatch denial, and post-recovery no-ghost-write checks; all phases passed for temporary local streams. The runner measured framed BLAKE3 identities for artifact, Worker bundle and ingress CA, bound the served deployment, and observed enabled output gate plus successful conditional-write probe. It does not prove remote fleet admission or object-store durability.
- The isolated Nix `celld-boundaries` check passed executable core/provider/standalone tests and Cargo dependency trees, then compiled an injected forbidden runtime import in a disposable workspace and rejected its named dependency edge.
- Pinned native Cairn strict validation passed. Before status spec sync, traceability was 561/561 referenced; after status sync and links to actual production and native fixture behavior, 567/567 referenced with no missing or dangling markers. Proposal/design gates passed; the tasks gate and native archive complete this lifecycle without weakening accepted requirements or the active evaluator change.
