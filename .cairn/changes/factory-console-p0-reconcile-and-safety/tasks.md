## Phase 1: Reconcile Branches & Combine Fleet UI

- [ ] [serial] Reconcile and fold `kiln-ui-simplify` (`xzunyskulwsq`) and source-bound owner projection groundwork from `kiln-factory-console` (`sxvupyosvrrq`) onto one clean baseline without duplicating panel frameworks. r[kiln.web.factory_p0_rail_promotion]
- [ ] [parallel] Promote `2.7 Cairn changes` and `2.8 Fleet` out of `<details> More work` in `web.html` (moving `triage` and `workviews` into the collapsed group) while preserving all 23 `parent: null` registrations and keeping `tests/live-console-ui.test.mjs` and `tests/console-inventory.test.mjs` green. r[kiln.web.factory_p0_rail_promotion]
- [ ] [parallel] Combine `kiln-fleet` standalone UI (`crates/kiln-fleet/src/ui.html`, `crates/kiln-fleet/src/api.rs`) into the unified `kiln-web-service` Factory Control Plane so `/` and `/ui` direct operators to the unified console instead of serving a second divergent HTML app. r[kiln.fleet.unified_console_surface]

## Phase 2: Control Safety & Auth Continuity

- [ ] [parallel] Replace all `window.prompt()` and `window.alert()` calls and unconfirmed claim release actions in `fleet.js` with the shared `#control-dialog` confirmation modal, reason input, and `CONTROL_FRESHNESS_MS` freshness guard. r[kiln.web.factory_p0_control_safety]
- [ ] [parallel] Remove `logout()` from the `pagehide` handler (`web.js`) while keeping explicit sign-out, token non-persistence (no URL/no `localStorage`), and network-identity rediscovery intact. r[kiln.web.session_continuity_pagehide]

## Phase 3: Unbury Backend Truth & Projections

- [ ] [parallel] Render `RunView.superseded_by`, `superseded_revision`, and `dirty_snapshot` in `web.js` `renderDetail()` and `runTableRow()` with an explicit invalidation/drift banner and 1-click jump or exact-revision rebuild action. r[kiln.web.run_supersession_and_reconcile]
- [ ] [parallel] Add `POST /api/runs/{id}/reconcile` in `crates/kiln-web-service/src/engine/api.rs` delegating to `Engine::refresh`, and render a first-class `Unknown` Reconciliation Card in `web.js` with a guardrail against blind reruns. r[kiln.web.run_supersession_and_reconcile]
- [ ] [parallel] Extend `crates/kiln-web-service/src/fleet.rs` `upstream_path` allowlist to proxy existing `kiln-fleet` `/v1/metrics` and `/v1/version` endpoints cleanly with positive and negative proxy tests. r[kiln.fleet.unified_console_surface]
- [ ] [parallel] Expose `queue_windows` and `WindowPolicy` bounds in `GET /api/code-review` (`crates/kiln-web-service/src/code_review.rs`) with positive and negative contract tests. r[kiln.review.queue.speculative.window_projection]
- [ ] [parallel] Add `GET /api/implementation/campaigns` in `crates/kiln-web-service/src/campaign_controls.rs` / `server/inventory.rs` and wire a selectable campaign list into Fleet/Workers views so operators never have to memorize or type campaign IDs. r[kiln.implementation.campaign.index_route]

## Phase 4: Verification

- [ ] [serial] Run `cargo test --locked -p kiln-web-service -p kiln-fleet`, Node console/inventory tests, and `cairn validate --root . --strict` covering valid paths, stale freshness guards, unsupported/unknown reconciliation states, malformed proxy paths, and empty campaign indices. r[kiln.web.factory_p0_control_safety]
