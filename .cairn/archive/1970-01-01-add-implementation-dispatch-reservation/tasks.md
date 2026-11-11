## Phase 1: Reservation

- [x] [serial] Add the deterministic dispatch reservation and idempotent re-reservation. r[kiln.implementation.dispatch_reservation.scenario.idempotent]
- [x] [serial] Refuse a changed plan/profile before transport. r[kiln.implementation.dispatch_reservation.scenario.conflict]
- [x] [serial] Require a live fence and claimable phase for reservation. r[kiln.implementation.dispatch_reservation]

## Phase 2: Dispatch and persistence

- [x] [serial] Require the reservation for acknowledgement and reconciliation. r[kiln.implementation.dispatch_reservation.scenario.missing]
- [x] [serial] Persist the reservation through the implementer shell and restart. r[kiln.implementation.dispatch_reservation.scenario.restart]
