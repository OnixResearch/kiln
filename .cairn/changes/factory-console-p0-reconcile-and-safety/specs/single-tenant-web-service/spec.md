## ADDED Requirements

### Requirement: Exact-revision supersession visibility and explicit Unknown run reconciliation

r[kiln.web.run_supersession_and_reconcile]
The live web service MUST expose `POST /api/runs/{id}/reconcile` delegating to `Engine::refresh` so operators can explicitly reconcile `Unknown` runs without blindly issuing a replacement rerun. The run list and run detail views in `web.js` MUST render serialized `superseded_by`, `superseded_revision`, and `dirty_snapshot` facts with an explicit invalidation or drift notice and a direct jump to the superseding run when present.

#### Scenario: Reconciling an Unknown run

r[kiln.web.run_supersession_and_reconcile.scenario.unknown]
- GIVEN a retained run whose outcome is `Unknown`
- WHEN the operator opens the run and invokes the Reconciliation Card action (`POST /api/runs/{id}/reconcile`)
- THEN the service MUST run provider reconciliation via `Engine::refresh` without creating a replacement run
- AND the UI MUST display the reconciliation result or retain `Unknown` honestly when the provider remains unreachable

#### Scenario: Superseded or dirty-snapshot run shows explicit binding drift

r[kiln.web.run_supersession_and_reconcile.scenario.superseded]
- GIVEN a retained run with `superseded_by` or `dirty_snapshot` set in `RunView`
- WHEN the operator views the run in the list or detail panel
- THEN the console MUST visibly mark the run as superseded or dirty-snapshot and link directly to `superseded_by`

### Requirement: Preserve authenticated sessions across browser pagehide

r[kiln.web.session_continuity_pagehide]
The console MUST NOT invoke `logout()` from the `pagehide` event handler. Explicit operator sign-out MUST still clear in-memory credentials, and bearer tokens MUST continue to avoid URL parameters and persistent `localStorage`.

#### Scenario: Browser sleep or tab transition preserves operator session

r[kiln.web.session_continuity_pagehide.scenario.preserve]
- GIVEN an authenticated operator monitoring live Factory state
- WHEN the browser fires `pagehide` during sleep, navigation, or bfcache transition
- THEN the console MUST NOT automatically clear the session token or force re-authentication
- AND clicking explicit sign-out MUST still clear credentials immediately
