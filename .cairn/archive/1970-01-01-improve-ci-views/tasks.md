## Phase 1: Implementation

- [x] [parallel] Failure-first run view, visible Rerun/Cancel, runs table and live run progress. r[kiln.web.failure_first_run] r[kiln.web.runs_table] r[kiln.web.live_progress]
- [x] [parallel] Project detail page with timeline, streak, duration trend and last success/failure. r[kiln.web.project_page]

## Phase 2: Verification

- [x] [serial] Real-data review at three widths and both appearances, a native active-run exercise, helper tests, repository gates, deployment and retained evidence. r[kiln.web.failure_first_run] r[kiln.web.project_page] r[kiln.web.runs_table] r[kiln.web.live_progress]

Evidence: `evidence/verification.json` and slice screenshots. A real in-progress run was not observed in the browser; progress was exercised with a browser-only simulated active state. Accepted and deployed on operator request 2026-10-29.
