## Why

The webhook queue and the implementer ledger existed separately. A scheduler
consumer is needed to move authenticated deliveries into admitted work without
bypassing verification, policy or deduplication.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `drain_implementation_queue`, which reads a bounded queue batch, verifies
  each stored signature again, normalizes through operator policy, admits into
  the durable implementer ledger, and removes the record only after a terminal
  decision.
- Keep forged or unverifiable records queued while returning an error.
- Document the consumer and remaining scheduler/dispatch gap.

## Impact

- **Files**: `crates/kiln-adapter-github/src/implementation.rs`,
  `crates/kiln-adapter-github/src/tests.rs`, `docs/implementation-work.md`.
- **Testing**: drain-once with duplicate delivery and forged-record retention.
- **Authority**: admission only. The consumer does not claim, dispatch, verify,
  publish or complete work.
