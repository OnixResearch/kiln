## Phase 1: Claim and fence

- [x] [serial] Add exactly-one-claim admission with frozen work identity, revision, ordinal and budget. r[kiln.implementation.claim]
- [x] [serial] Add monotonic fences and stale-fence refusal for every result transition. r[kiln.implementation.fencing]
- [x] [serial] Add bounded leases, renewal, expiry and replacement. r[kiln.implementation.lease]

## Phase 2: Dispatch and states

- [x] [serial] Add idempotent dispatch reconciliation with one observed workflow run and conflict refusal. r[kiln.implementation.dispatch]
- [x] [serial] Add persisted claimed/executing/candidate/verifying/blocked/failed/cancelled/superseded states with causes and failure classes. r[kiln.implementation.attempt_states]
- [x] [serial] Add abort or finish supersession rules without mutating the frozen contract. r[kiln.implementation.supersession]

## Phase 3: Verification

- [x] [serial] Test racing claims, stale fences, dispatch reconciliation, restart budget preservation, cancellation and supersession. r[kiln.implementation.attempt_states.scenario.cancel]
