## Why

Issue [#45](https://github.com/OnixResearch/kiln/issues/45) asks for a bounded worker completion/cancellation race matrix through the admitted provider-result boundary. The existing durable fixtures already cover both orderings of cancellation and completion across restart, the unknown-termination barrier, lost acknowledgements and the loss of authority on cancellation or supersession. Three cases remain without evidence: a late candidate handoff after cancellation at the handoff boundary itself, predecessor callbacks after a replacement attempt starts, and callbacks naming an attempt that was never admitted.

Mapping the predecessor case exposed a gap. `start_execution` and `reconcile_dispatch` adopt any observed workflow run for an attempt that has none recorded. A successor attempt can therefore adopt the run its predecessor executed, so one provider execution would read as the work of two attempts. The issue requires that an old provider identity or reused label cannot settle the successor.

## What Changes

- A provider workflow execution identity binds at most one attempt. Acknowledging or reconciling a run that another attempt already records is refused before any mutation.
- Pure ledger fixtures add the three missing race cases, restoring the serialized ledger between callbacks.
- The implementation runbook records the race coverage table, mapping every case of the matrix to its fixture or to the external owner.

## Capabilities

### Modified Capabilities

- `implementation-work`: one provider execution identity per attempt.

## Impact

`kiln-core` attempt ledger and its tests; `docs/implementation-runbook.md`. No storage format, schema or provider protocol changes. Existing ledgers whose attempts already share a run identity remain readable; only a new acknowledgement or reconciliation is refused.

## Non-goals

The Lattice implementation route and candidate-producing worker (Lattice #6/#7), live race qualification (#49), delegated-authority checks (#70) and the dependency-milestone residual of #45 are not part of this change.
