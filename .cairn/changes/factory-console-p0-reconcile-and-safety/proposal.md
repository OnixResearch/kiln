## Why

Kiln's web console currently buries its core Software Factory surfaces (`2.7 Cairn changes` and `2.8 Fleet`) inside a collapsed `More work` rail disclosure, splits fleet control across two incompatible UIs (`kiln-fleet/src/ui.html` and `kiln-web-service/src/fleet.js`), relies on unsafe `window.prompt()`/`window.alert()` dialogs and unconfirmed claim releases, hides already-serialized backend truth (`RunView.superseded_by`, `dirty_snapshot`, `queue_windows`, `/v1/metrics`, campaign index), lacks an operator `Unknown`-run reconciliation control, wipes bearer tokens on `pagehide`, and has two diverging unmerged UI branches (`kiln-ui-simplify` `xzunyskulwsq` and `kiln-factory-console` `sxvupyosvrrq`).

Before reshaping the full navigation topology in P1, P0 reconciles the unmerged UI branches, combines the standalone `kiln-fleet` UI into the unified console surface, closes the highest-leverage control safety and backend exposure gaps, and keeps all 23 registered destinations and existing inventory/UI test contracts green.

## What Changes

- Reconcile and fold `kiln-ui-simplify` (`xzunyskulwsq`) and the source-bound owner projection groundwork from `kiln-factory-console` (`sxvupyosvrrq`, `delivery_observations.rs`) onto one canonical branch without introducing parallel panel frameworks.
- Promote `2.7 Cairn changes` and `2.8 Fleet` out of the collapsed `More work` rail disclosure in `web.html` (moving `triage` and `workviews` into the collapsed group) while keeping all 23 `parent: null` destination registrations intact so `tests/live-console-ui.test.mjs` and `tests/console-inventory.test.mjs` remain green.
- Combine the standalone `kiln-fleet` UI (`crates/kiln-fleet/src/ui.html`) into the unified `kiln-web-service` Factory Control Plane so operators use one shared control plane surface and token system (`identity.css`) with zero bespoke divergent HTML UI or `prompt()`/`alert()` flows.
- Route Fleet claim/attempt mutations (`Renew`, `Release`, `Cancel`) through the shared `#control-dialog` confirmation modal with `CONTROL_FRESHNESS_MS` staleness checks and explicit reason capture.
- Render already-serialized `RunView.superseded_by`, `superseded_revision`, and `dirty_snapshot` fields in `web.js` with target-drift/invalidation banners and 1-click jump/rebuild actions.
- Expose `POST /api/runs/{id}/reconcile` (wrapping `Engine::refresh`) and surface a first-class `Unknown` Reconciliation Card on `Unknown` runs with an explicit guardrail against blind reruns.
- Extend `crates/kiln-web-service/src/fleet.rs` `upstream_path` allowlist to proxy already-existing `/v1/metrics` and `/v1/version` endpoints from `kiln-fleet`.
- Expose `queue_windows` and `WindowPolicy` bounds from `crates/kiln-web-service/src/code_review.rs` in `GET /api/code-review`.
- Add `GET /api/implementation/campaigns` in `campaign_controls.rs` / `server/inventory.rs` so operators can select active and recent campaigns without memorizing or typing campaign IDs.
- Remove `logout()` from the `pagehide` handler (`web.js`) so sleep/resume, tab switching, and navigation do not abruptly drop authenticated monitoring sessions.

## Impact

- **Files**: `crates/kiln-web-service/src/{web.html,web.js,ui.js,fleet.js,fleet.rs,code_review.rs,campaign_controls.rs,server/inventory.rs,engine/api.rs,delivery_observations.rs}`, `crates/kiln-fleet/src/{api.rs,ui.html}`, `crates/kiln-web-service/examples/explore.rs`, `docs/operator.md`, `tests/live-console-ui.test.mjs`, `tests/console-inventory.test.mjs`.
- **Testing**: `cargo test --locked -p kiln-web-service -p kiln-fleet`, Node console and inventory tests (`tests/live-console-ui.test.mjs`, `tests/console-inventory.test.mjs`), positive and negative tests for `POST /api/runs/{id}/reconcile`, `GET /api/implementation/campaigns`, `fleet.rs` proxy allowlist, `queue_windows` serialization, and combined `kiln-fleet` UI redirection/unification.
