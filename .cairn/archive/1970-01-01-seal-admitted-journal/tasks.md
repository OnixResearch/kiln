## Phase 1: Implementation

- [x] [serial] Seal `AdmittedJournal` fields, add read accessors and `with_review`, and update the shell. r[kiln.observe.dashboard.proof]
- [x] [serial] Update external dashboard tests, add the compile-fail proof check and modify the admitted-proof audit record. r[kiln.observe.dashboard.proof] r[kiln.observe.dashboard.proof.scenario.construct] r[kiln.github.event_proof.audit]

## Phase 2: Verification

- [x] [serial] Run dashboard and workspace tests, Clippy, rustfmt and Cairn validation; record evidence. r[kiln.observe.dashboard.proof] r[kiln.observe.dashboard.proof.scenario.read]
