## Why

The webhook shell authenticated issue deliveries but had no durable handoff to an
implementation scheduler. Losing a delivery forced polling recovery; storing raw
bytes in a bounded queue preserves the exact authenticated source for later
admission without granting authority.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `kiln_implementer::DeliveryQueue`, a bounded durable queue keyed by
  delivery identity.
- Add optional `implementation_queue` configuration to the GitHub service
  profile and Nickel contracts.
- In the webhook shell, authenticate `issues` deliveries and enqueue exact bytes;
  non-issue events and an absent queue keep prior behavior.
- Return duplicate status for repeated delivery IDs and fail closed when the
  queue is full.

## Impact

- **Files**: `crates/kiln-implementer/src/queue.rs`,
  `crates/kiln-adapter-github/src/{server,profile,tests}.rs`,
  `config/github-ci.ncl`, `config/github-contracts.ncl`,
  `docs/implementation-work.md`.
- **Testing**: idempotent queueing, forged signature refusal, non-issue
  pass-through, full-queue refusal, byte bound.
- **Authority**: queueing is not admission. A scheduler must still admit with
  operator policy, ledger dedup and claims.
