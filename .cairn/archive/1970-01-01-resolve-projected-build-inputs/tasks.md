## Phase 1: Planning

- [x] [serial] Derive Cargo layout facts in `kiln-build-graph` from the shared manifest parse, with tests over manifest fixtures and an unavailable layout. r[kiln.source.projection.build_inputs]
- [x] [serial] Add the pure build-input planner with the Cargo and Nix compatibility profiles, refusals and the plan identity, with positive and negative tests. r[kiln.source.projection.build_inputs] r[kiln.source.projection.build_inputs.scenario.planned] r[kiln.source.projection.build_inputs.scenario.refused] r[kiln.source.projection.build_inputs.scenario.nix] r[kiln.source.projection.build_inputs.scenario.applicability]

## Phase 2: Qualification

- [x] [serial] Author the typed build-input corpus with positive and negative fixtures and a checked export. r[kiln.source.projection.build_inputs]
- [x] [serial] Run the corpus through the planner, the projected materializer and the affected-project decision in a harness shared by an example and a test, executing planned commands behind `build-input-fixtures` with its Nix check, and retain the receipt. r[kiln.source.projection.build_inputs.scenario.qualified]
- [x] [serial] Document the profiles, their refusals and non-claims, and record verification evidence. r[kiln.source.projection.build_inputs]
