## Phase 1: Contract and dependencies

- [x] [serial] Add optional canonical request and evidence types, admission linkage and Nickel contract fields; keep absent-field bytes identical. r[kiln.radicle.canonical_source.evidence] r[kiln.radicle.canonical_source.evidence.scenario.links]
- [x] [serial] Link the vendored heartwood canonical crate into the adapter. r[kiln.radicle.canonical_source]

## Phase 2: Verification

- [x] [serial] Verify every delegate's signed refs, embedded identity bytes, namespaced targets and parents; compute the canonical head and compare it to the requested revision. r[kiln.radicle.canonical_source] r[kiln.radicle.canonical_source.identity] r[kiln.radicle.canonical_source.identity.scenario.revision]
- [x] [serial] Record canonical evidence in the receipt and recompute it after acquisition. r[kiln.radicle.canonical_source.evidence]

## Phase 3: Fixtures and verification

- [x] [serial] Add native 2-of-3, insufficient-vote and diverging-tip fixtures plus Nickel positive and negative receipts. r[kiln.radicle.canonical_source.scenario.quorum] r[kiln.radicle.canonical_source.scenario.insufficient] r[kiln.radicle.canonical_source.scenario.diverging]
- [x] [serial] Run contract, adapter and native tests, clippy, formatting, the Nickel check and the workspace check; document the mode and record evidence. r[kiln.radicle.canonical_source]
