# Tasks: let the HTML dashboard report update live

## Phase 1: Implementation

- [x] [serial] T1 Add the reload interval to the HTML renderer and emit a document reload instruction without a script. r[kiln.observe.dashboard.html.live_update]
- [x] [parallel] T2 Rewrite the report file atomically and derive the instruction's interval from the profile's watch interval. r[kiln.observe.dashboard.html.live_update.scenario.match]
- [x] [parallel] T3 Assert that an unwatched document stays a snapshot. r[kiln.observe.dashboard.html.live_update.scenario.snapshot]

## Phase 2: Verification and completion evidence

- [x] [serial] V1 Run the crate tests, the workspace gates, and the Cairn gates. r[kiln.observe.dashboard.html.live_update]
- [x] [serial] V2 Record the evidence, sync the accepted spec, archive the change, and publish the tip with receipts. r[kiln.observe.dashboard.html.live_update.nonclaims.scenario.scope]
