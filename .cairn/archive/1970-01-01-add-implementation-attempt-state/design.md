## Decisions

The ledger is a single-writer pure state machine. Exactly one claim per
canonical work key is enforced by the `current` map, and every mutating
transition requires the current unexpired fence. A stale process cannot mutate
or publish regardless of whether it can be stopped.

Fence and ordinal are separate: the fence increases globally for every claim,
while the ordinal is the attempt number within one work lineage and is bounded by
the work budget. Attempt identity is BLAKE3 over the canonical work key, ordinal
and fence, so it is deterministic and never reused.

Expiry blocks an active attempt and clears the current claim. The consumed
budget is not refunded; a replacement is admitted only when the lineage still has
budget. This prevents infinite retry of an unchanged failing plan.

Supersession never rewrites the frozen work identity or revision on an attempt.
`AbortActive` transitions the attempt to `Superseded` and revokes the fence;
`FinishActive` records the new revision and lets the attempt finish.

Work identity, attempt identity, workflow run and candidate identity are
separate fields. No CI run or rerun identity enters this ledger.

## Non-claims

The ledger does not execute a workflow, cancel a Lattice run, create a workspace,
verify a candidate, publish a pull request, or grant release authority. It is
serializable state that a durable shell and Lattice adapter still need to wire.
