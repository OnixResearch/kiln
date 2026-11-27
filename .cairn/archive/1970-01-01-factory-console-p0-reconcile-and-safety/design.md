## Context

`kiln-web-service` serves 23 destinations registered in `ui.js` and checked by `tests/console-inventory.test.mjs` and `tests/live-console-ui.test.mjs`. Meanwhile `kiln-fleet` serves an independent single-file `src/ui.html` with different visual tokens, 2s parallel polling, and blocking `prompt()`/`alert()` dialogs, and two unmerged `jj` commits (`xzunyskulwsq` and `sxvupyosvrrq`) modify the console navigation and add `factory-*.js` views. Several core Kiln facts—exact-revision run supersession, `dirty_snapshot`, `Unknown` run reconciliation, `queue_windows`, `/v1/metrics`, and active campaign listings—already exist in Rust structs or handlers but are omitted from HTTP projections or the browser DOM.

## Decisions

### Decision: Reconcile and fold `kiln-ui-simplify` and `kiln-factory-console` without breaking inventory invariants

**Choice:** Fold the navigation cleanup from `xzunyskulwsq` and owner-backed delivery observations (`delivery_observations.rs`) from `sxvupyosvrrq` into one coherent P0 baseline, keeping all 23 `parent: null` destination entries in `ui.js` and `docs/console-port-inventory.json` while promoting `2.7 Cairn changes` and `2.8 Fleet` out of `<details> More work` in `web.html`.

**Rationale:** `tests/live-console-ui.test.mjs:237-240` enforces `parent.number === place.number` and `tests/console-inventory.test.mjs:68` enforces exact number parity against `docs/console-port-inventory.json`. Promoting `Cairn` and `Fleet` in the rail markup without mutating `parent` keeps every contract green in P0 and defers the deliberate 23-to-7 destination consolidation to P1.

### Decision: Combine `kiln-fleet` standalone UI into the unified `kiln-web-service` Factory Control Plane

**Choice:** Retire the divergent bespoke UI in `crates/kiln-fleet/src/ui.html` and combine fleet UI access into the unified `kiln-web-service` control plane (configuring `kiln-fleet`'s `/` and `/ui` HTML endpoints to redirect/point to the configured `kiln-web-service` Factory console URL and returning a clean typed error/pointer when no console URL is configured, rather than maintaining a second independent web UI).

**Rationale:** Maintaining two incompatible HTML/JS frontends (`kiln-fleet/src/ui.html` vs `kiln-web-service/src/fleet.js`) duplicates state, drifts on visual tokens, bypasses confirmation/freshness safety rails, and multiplies polling load against `kiln-fleet`'s `MAX_CONNECTIONS = 32` listener.

### Decision: Explicit `Unknown` run reconciliation and shared `#control-dialog` safety rails

**Choice:** Add `POST /api/runs/{id}/reconcile` in `engine/api.rs` backed by `Engine::refresh`, render a dedicated `Unknown` Reconciliation Card in `web.js` alongside `superseded_by` / `dirty_snapshot` banners, and route every Fleet claim/attempt mutation through `#control-dialog` with `CONTROL_FRESHNESS_MS` staleness checks instead of `window.prompt()`/`window.alert()`.

**Rationale:** Kiln's core invariant treats `Unknown` as an explicit non-failure state that must never be blindly retried. Giving operators an explicit probe/reconcile action and guarded confirmation dialogs brings Fleet and CI controls to the same safety standard.

### Decision: Expose existing backend projections (`campaigns`, `/v1/metrics`, `queue_windows`) and preserve sessions across `pagehide`

**Choice:** Add `GET /api/implementation/campaigns`, extend `fleet.rs` `upstream_path` for `/v1/metrics` and `/v1/version`, serialize `queue_windows` + `WindowPolicy` in `GET /api/code-review`, and remove `logout()` from `pagehide` (`web.js:3446`) while retaining explicit sign-out and idle/expiry policy.

**Rationale:** Each projection already exists in backend state; exposing them eliminates manual campaign-ID memorization, unlocks live P1 Cockpit and Speculative Train rendering, and stops browser sleep/navigation from dropping authenticated monitoring sessions.

## Risks / Trade-offs

- Removing `kiln-fleet/src/ui.html` standalone interactive forms means operators interacting with a standalone `kiln-fleet` daemon without `kiln-web-service` must use `kiln-fleet` CLI or configure the console URL; CLI parity remains intact and tested.
- Folding `sxvupyosvrrq` (`factory-*.js`) must avoid duplicating `fleet.js` / `cairn.js` renderers; only source-bound owner projection helpers (`delivery_observations.rs`) and non-overlapping improvements should land in P0, while the full Cockpit surface lands in P1.
- Removing `pagehide` auto-logout requires documenting explicit sign-out and verifying bearer tokens still never persist to `localStorage` or URLs.
