## Decisions

The queue stores the signature and event name with the body so the consumer can
re-verify exactly as the receiver did. Removing a record only after a terminal
decision keeps unverifiable records visible for inspection instead of dropping
them.

The consumer is a library function rather than a new daemon: scheduling cadence,
concurrency and batch size belong to the operator shell. Batching is bounded by
the caller's `limit` and the queue bound.

Admission uses the same `ImplementationIssuePolicy` and `WorkLedger` as webhook
normalization, so an issue observed through either path resolves to one work
version.

## Non-claims

Draining is not claiming or dispatch. A claimed attempt and a Lattice execution
are still required before any implementation runs.
