## Phase 1: Data

- [x] [serial] Add `first_change_message` and `latest_decision_at_ms` to each `/api/reviews` summary, bounded and null when the native report cannot be read, with positive and negative Rust tests. r[kiln.dashboard.reviews.view] r[kiln.dashboard.reviews.view.scenario.find]

## Phase 2: Presentation

- [x] [serial] Add `receipts.js` with the decision vocabulary, tally, native-context reader, list rows and filters as pure tested helpers, and the page, receipt sheet and run-tab renderers. Remove the receipt code from `web.js` and route the page and the run tab through the module. r[kiln.dashboard.reviews.view] r[kiln.dashboard.reviews.view.scenario.retained] r[kiln.dashboard.reviews.view.scenario.decisions] r[kiln.dashboard.reviews.view.scenario.context]
- [x] [serial] Serve `/receipts.js` and `/receipts.css`, reduce the page's static markup to one container, restore `receipt_q` and `receipt_decision` from the URL, move `copyText` into `ui.js`, and update the register purpose and the Guide's vocabulary. r[kiln.dashboard.reviews.view.scenario.find] r[kiln.dashboard.reviews.view.scenario.absent]
- [x] [serial] Show a run's declared-but-unavailable review report as unavailable, and offer the exact retained bytes for download. r[kiln.dashboard.reviews.view.scenario.absent]

## Phase 3: Verification

- [x] [serial] Run the node rail, `cargo test --locked -p kiln-web-service`, Clippy and rustfmt for the changed Rust, lifecycle validation and traceability coverage with this change's delta root selected. r[kiln.dashboard.reviews.view]
- [x] [serial] Review the real assets over the real Rust reviews API with receipts that cover every decision, light and dark, at 1440px and 390px, and record bounded evidence that names every rail not run. r[kiln.dashboard.reviews.view.scenario.decisions] r[kiln.dashboard.reviews.view.scenario.context] r[kiln.dashboard.reviews.view.scenario.find]
- [x] [serial] Update the operator documentation and the changelog. r[kiln.dashboard.reviews.view]

Verification and its limits: [verification evidence](evidence/verification.json). Lifecycle checks: [lifecycle evidence](evidence/lifecycle.json). These records do not grant lifecycle acceptance, spec synchronization, publication or deployment authority.
