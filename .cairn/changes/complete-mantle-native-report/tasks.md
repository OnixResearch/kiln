## Phase 1: Contracts

- [x] [serial] Add the logical store prefix to the Mantle runtime profile and dispatch. r[kiln.mantle.native_report.store_binding]
- [x] [serial] Admit the complete pinned native report surface and reject drift. r[kiln.mantle.native_report.surface]

## Phase 2: Facts

- [x] [serial] Reconcile counts, failures, FOD mismatches, paths and cache facts. r[kiln.mantle.native_report.counts] r[kiln.mantle.native_report.paths] r[kiln.mantle.native_report.failures]
- [x] [serial] Project native collections and cache transfer facts into bounded metrics. r[kiln.mantle.native_report.facts]

## Phase 3: Verification

- [x] [serial] Add positive and negative report fixtures and focused tests. r[kiln.mantle.native_report.surface] r[kiln.mantle.native_report.counts]
- [x] [serial] Update documentation and record verification evidence. r[kiln.mantle.native_report.surface]
- [x] [serial] Run focused/full Cargo, Clippy, rustfmt and Cairn validation. r[kiln.mantle.native_report.surface]
