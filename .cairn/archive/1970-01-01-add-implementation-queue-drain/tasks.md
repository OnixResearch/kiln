## Phase 1: Consumer

- [x] [serial] Drain a bounded queue batch, re-verify each delivery and admit through operator policy. r[kiln.implementation.queue_drain]
- [x] [serial] Remove a record only after admission or an explicit terminal decision; keep forged records queued. r[kiln.implementation.queue_drain.scenario.forged]

## Phase 2: Verification

- [x] [serial] Test duplicate delivery resolving to one work version and forged record retention. r[kiln.implementation.queue_drain.scenario.once]
