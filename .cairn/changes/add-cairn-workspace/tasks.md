## Phase 1: Snapshot and projection

- [x] [serial] Add the bounded canonical snapshot to `kiln_implementer::observation` with positive and negative cases (missing tables, truncation, oversized records). r[kiln.cairn.workspace]
- [x] [serial] Add the pure Cairn workspace index and detail projection with identity, visibility, stage, evidence and blocker cases. r[kiln.cairn.workspace.scenario.identity] r[kiln.cairn.workspace.scenario.visibility] r[kiln.cairn.workspace.scenario.stages] r[kiln.cairn.workspace.scenario.evidence] r[kiln.cairn.workspace.scenario.blockers]
- [x] [serial] Add the read-only routes with role, method and query admission. r[kiln.cairn.workspace]

## Phase 2: Console and exploration

- [x] [serial] Add the Cairn changes page (2.7) with URL-bound list and detail, late-response protection and campaign links, with Node UI tests. r[kiln.cairn.workspace.scenario.reload]
- [x] [serial] Seed canonical work and attempts in the exploration workspace, add its catalog example and review it in a browser. r[kiln.cairn.workspace.scenario.example]

## Phase 3: Verification

- [ ] [serial] Update the operator guide and changelog, register the active delta root, run focused, workspace and Nix gates, and pass Cairn gates, validation and traceability. r[kiln.cairn.workspace]
