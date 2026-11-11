## Phase 1: Durable store

- [x] [serial] Add the SQLite-backed work/attempt store with versioned rows and immediate mutations. r[kiln.implementation.durable_store]
- [x] [serial] Expose admission, claim_next, leases, dispatch, candidate, failure, cancel and supersede transitions. r[kiln.implementation.durable_store]

## Phase 2: Recovery and verification

- [x] [serial] Add recover() that expires leases and reports the active reconciliation set. r[kiln.implementation.recovery]
- [x] [serial] Test concurrent claims, restart dedup/budget, stale fence and dispatch reconciliation across restart. r[kiln.implementation.durable_store.scenario.concurrent] r[kiln.implementation.durable_store.scenario.restart] r[kiln.implementation.recovery.scenario.restart]
