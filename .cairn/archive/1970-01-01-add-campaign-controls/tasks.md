## Phase 1: Owner commands

- [x] [serial] Add `CampaignState::decision_in_current_scope`, share it with `add_decision` and refuse changed-scope resolutions, with positive and negative core cases. r[kiln.implementation.campaign.controls.scenario.decisions]
- [x] [serial] Share in-transaction start, transition and issuance helpers in `CampaignStore`, add the durable operator command record with typed failures and `apply_operator`, and add `pause` to the campaign operator. Cover replay, request conflicts, stale versions, existing campaigns, admission states, decision answers and restart over SQLite. r[kiln.implementation.campaign.controls] r[kiln.implementation.campaign.controls.scenario.replay] r[kiln.implementation.campaign.controls.scenario.admission] r[kiln.implementation.campaign.controls.scenario.decisions]

## Phase 2: Service controls

- [x] [serial] Add `config/web-campaign-controls.ncl` with positive and negative fixtures and `campaign_controls.rs`: profile admission, grants, definition previews, the inbox and command execution. Wire `--campaign-controls` and the three routes into the server, including denial audit. r[kiln.implementation.campaign.controls] r[kiln.implementation.campaign.controls.scenario.start] r[kiln.implementation.campaign.controls.scenario.endpoints] r[kiln.implementation.campaign.controls.scenario.conflict] r[kiln.implementation.campaign.controls.scenario.roles]
- [x] [serial] Cover viewer, worker, granted and ungranted operators, expiry, removed grants, review audience, changed definitions, unsupported endpoints, replay and restart in web-service tests. r[kiln.implementation.campaign.controls.scenario.roles] r[kiln.implementation.campaign.controls.scenario.conflict] r[kiln.implementation.campaign.controls.scenario.replay]

## Phase 3: Console and exploration

- [x] [serial] Render admission controls, the decision inbox and start previews in the campaign panel with request-ID retries and conflict reloads, with Node UI tests. r[kiln.implementation.campaign.controls.scenario.console]
- [x] [serial] Add the control profile, decisions and definitions to the exploration workspace and its catalog, and review the console in a browser. r[kiln.implementation.campaign.controls.scenario.console] r[kiln.implementation.campaign.controls.scenario.start]

## Phase 4: Verification

- [x] [serial] Update the operator guide, runbook and changelog; register the active delta root; run focused, workspace and Nix gates; and pass Cairn gates, validation and traceability. r[kiln.implementation.campaign.controls]
