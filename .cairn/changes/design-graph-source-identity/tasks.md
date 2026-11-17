## Phase 1: Contract

- [x] [serial] Specify identities, relations, import and export, authority modes and evidence binding in the proposal, design and delta spec. r[kiln.source.graph.identity] r[kiln.source.graph.relations] r[kiln.source.graph.interchange] r[kiln.source.graph.authority]

## Phase 2: Model and fixtures

- [ ] [serial] Add the pure identity and relation model with fixtures for aliases, copies, unrelated equal-content files, edits, renames and the three removal outcomes. r[kiln.source.graph.identity.scenario.equal_bytes] r[kiln.source.graph.identity.scenario.rename] r[kiln.source.graph.relations.scenario.removal]
- [ ] [serial] Add the import ledger and deterministic export with ambiguity, unsupported-object refusal and round-trip fixtures. r[kiln.source.graph.interchange.scenario.ambiguous] r[kiln.source.graph.interchange.scenario.round_trip]
- [ ] [serial] Add authority-mode records with expected-state publication refusal and evidence invalidation fixtures. r[kiln.source.graph.authority.scenario.conflict]

## Phase 3: Prototype

- [ ] [serial] Demonstrate one graph snapshot, two project layouts and one shared entity with snapshot reproduction after restart, without converting or publishing any repository. r[kiln.source.graph.identity]
