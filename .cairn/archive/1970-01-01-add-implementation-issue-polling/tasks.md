## Phase 1: Port and normalization

- [x] [serial] Add the bounded issue-poll transport port and request/page types. r[kiln.implementation.polling]
- [x] [serial] Normalize polled records through the shared work contract with content-bound identities. r[kiln.implementation.polling.scenario.pages]

## Phase 2: Bounds and verification

- [x] [serial] Enforce enrollment, text, page and record bounds and fail instead of truncating recovery. r[kiln.implementation.polling.scenario.bound]
- [x] [serial] Test deterministic ordering, duplicate sequence selection and stable identity. r[kiln.implementation.polling.scenario.pages]
