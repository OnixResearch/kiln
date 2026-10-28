## Phase 1: Admission

- [x] [serial] Validate proposal, design and task gates before implementation. r[kiln.dashboard.usability]

## Phase 2: Repair

- [x] [parallel] Repair responsive presentation, navigation and readable summaries. r[kiln.dashboard.usability]
- [x] [parallel] Repair source, COB and retained review browsing. r[kiln.dashboard.usability]
- [x] [parallel] Repair guided run discovery, inspection, logs, evidence and comparison. r[kiln.dashboard.usability]

## Phase 3: Verification

- [x] [serial] Exercise all pages and discovered failure paths with the real browser and update operator guidance. r[kiln.dashboard.usability]

## Observed verification

- Current frontend HTML/CSS/JavaScript was served through browser-only asset overrides against the authenticated live read-only API. No deployment, restart, execution mutation or publication was performed.
- Walked all twelve workspace pages and all eight selected-run views. Overview failure filters showed seven matching retained runs; source and retained-run identities remained explicit. Opening Logs read the selected stream, including empty stdout and real captured stderr.
- Same-project comparison offered four alternatives for the retained Kiln run and rendered recorded outcomes/source/timing with unavailable historical values preserved. Evidence suggestions loaded and inspected an exact run.
- Read a real COB and retained review receipt. Blocked inventory GET requests independently: exact selected details still loaded. A missing receipt stayed selected with a not-found message; browser Back restored the real receipt. Clearing a COB removed its prior content.
- Source verification reproduced stale diff content before repair, then exercised repository/revision/file selection and clearing. A delayed previous-repository tree response did not overwrite the newly selected repository.
- Desktop light/dark and 390px mobile surfaces were inspected. Drawer Tab/Shift-Tab wrapping, Escape, inert closed navigation, backdrop and page navigation were exercised. Run-title scroll placed the heading below the fixed header. Opening and dismissing rerun confirmation did not dispatch a run.
- Clearing the session hid the workspace and removed exact facts, retained-run suggestions, COB and receipt content. No browser JavaScript page errors were observed in the integrated pass. Browser tabs were closed.
- Operator guidance is in `docs/operator.md` under **Navigate the live workspace**. Kiln application/dashboard maintainers own these behaviors. These observations do not establish deployed-byte identity, exhaustive accessibility or backend correctness.
- Checks passed: `node --check crates/kiln-web-service/src/web.js`; `nix develop path:. -c cargo test --locked -p kiln-web-service`; `nix develop path:. -c cargo clippy --locked -p kiln-web-service --all-targets -- -D warnings`; Cairn proposal/design/task gates and repository validation with the explicit generated policy. Kache reported a host cache disk I/O problem and compiled without caching; checks completed successfully.
