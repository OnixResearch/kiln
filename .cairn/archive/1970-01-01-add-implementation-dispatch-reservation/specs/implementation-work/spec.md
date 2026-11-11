# Implementation Work Specification

## ADDED Requirements

### Requirement: Durable dispatch reservation

r[kiln.implementation.dispatch_reservation] Before provider transport, an
attempt MUST reserve exactly one dispatch binding over its identity, current
fence, provider profile and admitted plan identity. The reservation MUST carry a
deterministic effect identity and idempotency key. Repeating an equal reservation
MUST be idempotent; a different plan or profile for the same live attempt, a
stale fence, an expired lease, or a non-claimable phase MUST fail before
transport. The reservation MUST persist across restart. Dispatch acknowledgement
and reconciliation MUST require the reservation, accept an equal observed
workflow run idempotently, and refuse a conflicting run.

#### Scenario: Equal reservation is idempotent

r[kiln.implementation.dispatch_reservation.scenario.idempotent]
- GIVEN a claimed attempt
- WHEN the same provider plan is reserved more than once
- THEN one reservation MUST be retained with its original reservation time and identities

#### Scenario: Conflicting reservation fails

r[kiln.implementation.dispatch_reservation.scenario.conflict]
- GIVEN a claimed attempt with a reservation
- WHEN a different plan or provider profile is reserved
- THEN the reservation MUST fail before transport
- AND the original reservation MUST remain unchanged

#### Scenario: Reservation survives restart

r[kiln.implementation.dispatch_reservation.scenario.restart]
- GIVEN a reservation recorded before transport
- WHEN the durable ledger is reopened
- THEN the same reservation MUST be available
- AND execution acknowledgement MUST use it without creating a second reservation

#### Scenario: Acknowledgement requires reservation

r[kiln.implementation.dispatch_reservation.scenario.missing]
- GIVEN a claimed attempt without a reservation
- WHEN execution acknowledgement or reconciliation is requested
- THEN it MUST fail
- AND no workflow run may be recorded
