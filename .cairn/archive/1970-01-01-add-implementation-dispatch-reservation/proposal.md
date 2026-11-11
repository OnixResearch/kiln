# Add a durable implementation dispatch reservation

## Why

Kiln records a provider workflow only after dispatch, so a crash between
transport and acknowledgement can redispatch the same attempt without a durable
intent record. Issue OnixResearch/kiln#45 requires the claim-to-dispatch
transition to reserve the effect before transport and reconcile ambiguity
without blind duplicate execution.

## What Changes

- Add `DispatchReservation` to the attempt ledger with deterministic effect and
  idempotency identities over the attempt, fence, plan and provider profile.
- Require a reservation before workflow acknowledgement or reconciliation.
- Make equal reservations idempotent and refuse conflicting plans, stale fences,
  expired leases and non-claimable phases.
- Persist reservations in the existing versioned attempt row so restart and
  shell wrappers preserve them.
- Document the reservation and add pure/durable tests.

## Impact

- **Files**: `crates/kiln-core/src/implementation/attempt.rs`,
  `crates/kiln-implementer/src/lib.rs`, durable tests, docs and specification.
- **Testing**: idempotent reservation, conflict, stale fence, missing
  reservation, execution acknowledgement and restart.
- **Authority**: reservation only. Provider transport, workspace execution and
  candidate verification remain external.
