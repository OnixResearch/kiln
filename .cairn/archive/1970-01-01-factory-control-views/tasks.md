## Phase 1: Implementation

- [x] [serial] Compose Factory triage, change cockpit, recovery, delivery, workflow and flow views. r[kiln.web.factory_control_views]
- [x] [serial] Verify owner boundaries and unavailable observations with disposable browser fixtures. r[kiln.web.factory_control_views]

## Evidence

- `node --test tests/live-console-ui.test.mjs`: 90 passed. `cargo test --locked -p kiln-web-service --lib delivery_observations`: duplicate targets refused and stale readback never borrows the intended revision.
- Chromium against the disposable explorer: Fleet claims returning HTTP 503 did not hide CI or Cairn; crossed work and attempt IDs showed no recovery control; the exact attempt read its retained evidence and refused reconciliation without a seal. Workflow selectors preserved exact revisions. Stale Delivery observations displayed recorded health and did not infer a missing observed revision.
- No live producer was enrolled and no deployment, replacement dispatch, or provider mutation was exercised.
