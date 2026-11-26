## Phase 1: Causal Event Timeline, Failure Digest & Gate Passport

- [x] [parallel] Replace raw `renderRecorded()` JSON dumps in Run `Activity` and `Evidence` (`web.js`, `inventory.js`) with a structured Causal Event-Sourced Decision Timeline (`Admitted -> Gate -> Dispatched -> Observations -> Controls/Unknown -> Terminal`) featuring timestamps, principal/agent attribution, and copyable 12-char BLAKE3 `event_identity` badges. r[kiln.web.causal_timeline_and_failure_digest]
- [x] [parallel] Add a deduplicated Cross-Node Failure Digest above `Logs` in `Runs & Evidence` (`web.js`) summarizing failing nodes/steps across multi-node DAGs with 1-click jump to each failing stream and line. r[kiln.web.causal_timeline_and_failure_digest]
- [x] [parallel] Build the unified Gate & Receipt Passport across `Runs & Evidence`, `Code Review & Queue`, and read-only `kiln-dashboard` uniting CI checks, `cairn-authoritative-completion` receipts, Patchgate decisions (`receipts.js`), SLSA/in-toto statements, Plan Review annotations, and Semantic Merge O/A/B/M quartets with explicit provenance badges (`Observed Receipt` vs `Agent Claim` vs `Declared`). r[kiln.cairn.gate_passport_surface]
- [x] [parallel] Add Plannotator-style step-level Plan Review annotation and feedback UI (`docs/plan-review.md`) wired to plan-review receipts inside the Slide-Over and Delivery views. r[kiln.implementation.plan_review_console_surface]

## Phase 2: Fair-Admission Governance, Recovery & Atlas Topology

- [x] [parallel] Expose `GET /api/implementation/admission` and guarded fair-admission quota/share views plus live `AttemptMetrics` token/turn/wall-time burn panels in `3.1 Fleet & Workers` (`fleet.js`, `fleet-capacity.js`). r[kiln.admission.live_console_governance]
- [x] [parallel] Expose owner-delegated publication and integration recovery controls (`prepare-publication`, `publish`, `recover-integration`, `close-source`) behind Tier 2/3 safety rails in `2.1 Delivery & Cairn` (`cairn.js`), failing closed on stale candidate identities or missing operator authority. r[kiln.cairn.gate_passport_surface]
- [x] [parallel] Implement opt-in Atlas declared-vs-observed topology views (`docs/atlas-view.md`) strictly bounded to `<=64` nodes using same-origin `dag-canvas` / flat graph fallbacks. r[kiln.web.causal_timeline_and_failure_digest]

## Phase 3: Static Asset Transport & Module Test Hardening

- [x] [parallel] Add `ETag` (`304 Not Modified`), `Cache-Control`, and `gzip`/`zstd` encoding (respecting `Accept-Encoding` and `Vary`) for static console assets in `crates/kiln-web-service/src/server.rs` to eliminate the 33-request / 1.27 MB uncompressed cold load. r[kiln.web.static_asset_caching_and_compression]
- [x] [parallel] Add dedicated Node module tests covering `web.js`, `fleet.js`, `work.js`, and `project.js` and register them in `docs/console-port-inventory.json`. r[kiln.web.static_asset_caching_and_compression]

## Phase 4: Verification

- [x] [serial] Run `cargo test --locked -p kiln-web-service -p kiln-dashboard`, all Node console/inventory/module tests, negative tests (stale Gate Passport candidates, Atlas `>64`-node rejection/fallback, invalid `Accept-Encoding` / `If-None-Match` headers, unauthorized fair-admission mutations), and `cairn validate --root . --strict`. r[kiln.web.static_asset_caching_and_compression]
