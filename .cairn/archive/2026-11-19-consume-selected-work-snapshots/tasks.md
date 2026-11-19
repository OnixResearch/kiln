## Phase 1: Producer import

- [x] [serial] Re-import `cairn-core` with its generated policies and transitive artifact and historical kiln-core imports through the repo-owned importer, with root manifest patches and source records. r[kiln.selected_work.admission]

## Phase 2: Core model

- [x] [serial] [depends:kiln.selected_work.admission] Add provider-neutral selected-work facts, typed references, obligation plan and guidance projection to `kiln-core`. r[kiln.selected_work.references]
- [x] [serial] [depends:kiln.selected_work.references] Add immutable attempt contracts, refresh/replan transitions, current/historical/unavailable history and campaign member retention. r[kiln.selected_work.attempts]
- [x] [serial] [depends:kiln.selected_work.attempts] Add the obligation completion decision over canonical checks and authoritative completion, and the restricted display projection. r[kiln.selected_work.completion]

## Phase 3: Adapters and surfaces

- [x] [serial] [depends:kiln.selected_work.references] Add the `cairn selected-work` reader, producer admission and mapping, typed refusals and the explicit legacy compatibility profile. r[kiln.selected_work.admission.scenario.refused]
- [x] [serial] [depends:kiln.selected_work.completion] Carry the retained obligation projection through worker context, campaign inspection and the UI projection. r[kiln.selected_work.completion.scenario.surfaces]
- [x] [serial] [depends:kiln.selected_work.admission] Derive typed-intake member stages from the admitted snapshot's native readiness, blockers, units and dependencies, keeping the legacy reader only under the compatibility profile. r[kiln.selected_work.stages]
- [x] [serial] [depends:kiln.selected_work.admission] Host-normalize admitted obligations into the Lattice workflow intent and admit compiled plans with proposal refusal. r[kiln.selected_work.lattice]

## Phase 4: Fixtures and evidence

- [x] [serial] [depends:kiln.selected_work.completion] Retain producer/consumer fixtures and the finite verify pilot with refusal cases. r[kiln.selected_work.completion.scenario.pilot]
- [x] [serial] [depends:kiln.selected_work.display] Document the consumer boundary and run the repository gates. r[kiln.selected_work.display]
