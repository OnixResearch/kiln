## Phase 1: Presentation

- [x] [serial] Add timezone-labelled browser-local timestamps alongside UTC and expose last-run completion without inventing missing timing. r[kiln.dashboard.time.local]

## Phase 2: COB operation

- [x] [serial] Exercise six-type read conformance and configure only the existing public Kiln store through the separate typed read-only policy; preserve profiles, CI history and native refs. r[kiln.dashboard.cobs.operated]
- [x] [serial] Import every active Cairn change into the selected store with the operator's existing identity, verify only namespaced COB refs were added and that existing refs, HEAD and store config are unchanged. r[kiln.dashboard.cobs.population]

## Phase 3: Verification

- [x] [serial] Exercise installed timestamp and COB UI/API paths, run required checks and retain bounded observations with the operator procedure. r[kiln.dashboard.time.local] r[kiln.dashboard.cobs.operated] r[kiln.dashboard.cobs.population]

Observed outcomes, the pre-existing traceability hold and the unchanged
authority boundaries are retained in `evidence/verification.json`.
