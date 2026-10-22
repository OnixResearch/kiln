# Tasks: add a property-based terminal rail for the operator dashboard

## Phase 1: Cohort and specifications

- [x] [serial] T1 Pin the property-testing tool flake at one exact revision and record its terms in the license map as a separately executed program. r[kiln.observe.dashboard.property_rail.cohort]
- [x] [parallel] T2 Add the admitted, bounded, and rejected journal fixtures. r[kiln.observe.dashboard.property_rail.scenario.match]
- [x] [serial] T3 Write the admitted-journal specification with properties derived from the staged journal and the checked-in profile. r[kiln.observe.dashboard.property_rail]
- [x] [parallel] T4 Write the rejected-journal specification. r[kiln.observe.dashboard.property_rail.scenario.rejected]

## Phase 2: Wiring and policy

- [x] [serial] T5 Add the Nix check that runs the specifications over all three fixtures and keeps the traces. r[kiln.observe.dashboard.property_rail.scenario.bounded]
- [x] [parallel] T6 Extend the local traceability policy with the specification and flake evidence roots and regenerate the exported policy. r[kiln.observe.dashboard.property_rail.nonclaims]

## Phase 3: Verification and completion evidence

- [x] [serial] V1 Confirm every property can fail by running the specification against a deliberately wrong expectation. r[kiln.observe.dashboard.property_rail]
- [x] [serial] V2 Run the property rail, the workspace gates, the Nickel gate, and the Cairn gates and confirm they are clean. r[kiln.observe.dashboard.property_rail.nonclaims]
- [x] [serial] V3 Record the rail's evidence, its non-claims, and the tool revision, then sync and archive the change. r[kiln.observe.dashboard.property_rail.nonclaims.scenario.scope]
