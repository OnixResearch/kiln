## Phase 1: Core source status

- [x] [serial] Bind integration identity, work/version, attempt/fence and an admitted GitHub issue alias. r[kiln.implementation.source_status.scenario.close]
- [x] [serial] Refuse closure before integration or with mismatched source/evidence. r[kiln.implementation.source_status.scenario.refused]
- [x] [serial] Retain the closed receipt and publication `source-reconciled` phase. r[kiln.implementation.source_status]

## Phase 2: Provider

- [x] [serial] Read first, close once, and read back the exact issue. r[kiln.github.source_status.scenario.close]
- [x] [serial] Reuse an already closed issue without mutation. r[kiln.github.source_status.scenario.closed]
- [x] [serial] Add the `close-source` operator command. r[kiln.github.source_status]
