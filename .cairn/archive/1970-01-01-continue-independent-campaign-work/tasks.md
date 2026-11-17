## Phase 1: Selection

- [x] [serial] Add `CampaignState::evaluate`, `CampaignEvaluation`, `SelectionBasis` and per-member action, hold and deferred classification; keep barriers first, select the first member action, surface a hold only when no member can act, and gate writer claims on the writer slot. r[kiln.implementation.campaign.independent_progress]
- [x] [serial] Add core cases for the review-wait fixture, every member-identity ordering, member-local hold scope with and without dependency, exact review answers, and writer, pause, cancel and uncertain-effect barriers. r[kiln.implementation.campaign.independent_progress.scenario.review_wait] r[kiln.implementation.campaign.independent_progress.scenario.permutation] r[kiln.implementation.campaign.independent_progress.scenario.local_scope] r[kiln.implementation.campaign.independent_progress.scenario.review_gate] r[kiln.implementation.campaign.independent_progress.scenario.barriers]

## Phase 2: Driver and projection

- [x] [serial] Move the bounded loop to `drive_bounded` over `CampaignDriveShell`, keep native refresh, recovery and claim steps in `NativeCampaignConsumer`, recover executing members when no member can act, and add `basis` and `holds` to the drive output. r[kiln.implementation.campaign.independent_progress.scenario.driver]
- [x] [serial] Add store-backed scripted-owner cases for progress, restart without reclaim or redispatch, and refused canonical claims. r[kiln.implementation.campaign.independent_progress.scenario.driver] r[kiln.implementation.campaign.independent_progress.scenario.barriers]
- [x] [serial] Add `selection_basis` and per-member `selection_hold` to the projection, render holds in the console, update the fixture, and cover JSON, HTML and Node rendering. r[kiln.implementation.campaign.independent_progress.scenario.projection]

## Phase 3: Verification

- [x] [serial] Run core, implementer and web-service tests, the inventory Node rail, formatting and Clippy for the touched crates, and record the exercised scope. r[kiln.implementation.campaign.independent_progress]
- [x] [serial] Update the runbook, operator guide and changelog, register the active delta root, and pass Cairn gates, validation and active-delta traceability. r[kiln.implementation.campaign.independent_progress]
