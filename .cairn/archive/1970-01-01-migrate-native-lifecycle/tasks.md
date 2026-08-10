## Phase 1: Migration

- [x] [serial] Rebase the Kiln policy on the pinned Cairn policy and retain a narrow project overlay. r[kiln.lifecycle.native_store]
- [x] [serial] Preview and execute the hash-bound lifecycle migration. r[kiln.lifecycle.native_store.scenario.parity]
- [x] [serial] Update active repository references and remove the legacy lifecycle tree. r[kiln.lifecycle.native_store.scenario.active_paths]

## Phase 2: Verification

- [x] [parallel] Verify deterministic policy export and stale-plan rejection. r[kiln.lifecycle.native_store.scenario.stale_plan]
- [x] [parallel] Run workspace, Clippy, traceability, Cairn, and Nix checks. r[kiln.lifecycle.native_store.scenario.active_paths]
- [x] [serial] Sync the accepted requirement, record evidence, and archive the change. r[kiln.lifecycle.native_store]
