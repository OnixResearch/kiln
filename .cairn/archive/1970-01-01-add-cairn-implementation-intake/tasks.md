## Phase 1: Intake

- [x] [serial] Add bounded complete-snapshot intake with stable unit aliases and requirement acceptance. r[kiln.implementation.cairn_intake]
- [x] [serial] Produce no work for blocked, empty or completion-only outcomes. r[kiln.implementation.cairn_intake]
- [x] [serial] Refuse incomplete, incomplete-limit, forged or archive-ready selections. r[kiln.implementation.cairn_intake]
- [x] [serial] Validate policy identity, acceptance, paths and budget before intake. r[kiln.implementation.cairn_intake]

## Phase 2: Ledger and replay

- [x] [serial] Add the intake binary and durable-ledger admission. r[kiln.implementation.cairn_intake]
- [x] [serial] Exclude source versions from the canonical key so edits supersede one unit. r[kiln.implementation.cairn_intake]
