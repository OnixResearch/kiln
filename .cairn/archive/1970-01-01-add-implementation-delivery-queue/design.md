## Decisions

The receiver stores exact bytes and a delivery identity, not a normalized work
record. Normalization needs operator policy (acceptance, paths, effects, budget)
that belongs to the scheduler, not the webhook receiver. Storing exact bytes also
lets the consumer re-verify the signature before admission.

The queue uses a primary key on delivery ID, so redelivery is idempotent without
a separate replay table. It refuses when full instead of evicting, because silent
loss would make recovery look complete.

Queue membership is not admission: only the implementer shell's `admit` and
`claim_next` transitions grant a version or a claim.

## Non-claims

The webhook shell does not normalize, admit, claim or dispatch implementation
work. A separate scheduler and the Lattice implementation route remain required
for execution.
