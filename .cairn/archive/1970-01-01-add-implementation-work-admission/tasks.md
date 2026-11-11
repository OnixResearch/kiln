## Phase 1: Contract

- [x] [serial] Add the bounded versioned implementation-work contract with canonical identity and alias keys. r[kiln.implementation.contract]
- [x] [serial] Refuse unknown schemas, unenrolled identities, unsafe paths, unknown effects, missing acceptance or budget, and oversized fields. r[kiln.implementation.contract.scenario.refused]

## Phase 2: Admission semantics

- [x] [serial] Add explicit task-level mapping with forged/partial/repeated mapping refusal. r[kiln.implementation.mapping.scenario.forged]
- [x] [serial] Merge explicitly linked units and keep unrelated same-title work separate. r[kiln.implementation.mapping.scenario.linked]
- [x] [serial] Add dependency and acceptance blocking with cycle detection and blocked work excluded from ranking. r[kiln.implementation.eligibility.scenario.blocked]
- [x] [serial] Add deterministic ready ranking by priority and canonical key. r[kiln.implementation.eligibility.scenario.rank]

## Phase 3: Replay and evidence

- [x] [serial] Add duplicate, stale, ambiguous, and supersession semantics with retained history. r[kiln.implementation.replay.scenario.duplicate] r[kiln.implementation.replay.scenario.edit]
- [x] [serial] Keep archive readiness and lifecycle state out of admission. r[kiln.implementation.replay.scenario.lifecycle]
- [x] [serial] Run the focused core tests and document the boundary and remaining adapters. r[kiln.implementation.contract]
