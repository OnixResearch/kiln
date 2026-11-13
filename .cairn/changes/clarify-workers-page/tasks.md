## Phase 1: Presentation

- [x] [serial] Add `workers.js` with the shared state vocabulary/validator, pure target ordering/filter/selection/heartbeat/timing helpers, and register/detail rendering with guarded session-safe loading. Migrate both the Workers and Overview callers and remove the old renderer and filter handlers. r[kiln.dashboard.workers.view] r[kiln.dashboard.workers.view.scenario.restore] r[kiln.dashboard.workers.view.scenario.cleared]
- [x] [serial] Add `workers.css` using shared graphic tokens and table stacking, load/serve both assets, reduce static Workers markup, update the register purpose and remove obsolete Worker CSS. r[kiln.dashboard.workers.view.scenario.register] r[kiln.dashboard.workers.view.scenario.detail]
- [x] [serial] Add positive/negative Node cases for stable ordering, selection independent of filters, missing observations and expired heartbeat presentation. r[kiln.dashboard.workers.view.scenario.register] r[kiln.dashboard.workers.view.scenario.heartbeat]

## Phase 2: Verification

- [x] [serial] Run the Node rail, JS syntax checks, web-service Cargo tests, Rust formatting and Clippy, then record their exercised scope. r[kiln.dashboard.workers.view]
- [x] [serial] Review real assets with live read-only data and bounded synthetic scenarios across the designed appearances/widths; verify navigation, keyboard focus, errors and session clearing without worker effects. r[kiln.dashboard.workers.view.scenario.restore] r[kiln.dashboard.workers.view.scenario.cleared] r[kiln.dashboard.workers.view.scenario.detail] r[kiln.dashboard.workers.view.scenario.heartbeat]
- [x] [serial] Update operator documentation and changelog, and pass Cairn validation, policy freshness and active-delta traceability with bounded evidence and non-claims. r[kiln.dashboard.workers.view]
