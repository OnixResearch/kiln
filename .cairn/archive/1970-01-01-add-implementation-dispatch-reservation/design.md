## Decisions

The reservation lives on the attempt rather than a second ledger so it shares
the existing fence, lease, supersession and restart path. Its effect identity is
`dispatch-<BLAKE3>` and its idempotency key binds the same digest; both are
derived, not caller-supplied, so a caller cannot reserve two effects for one
plan by changing literals.

An equal plan/profile reservation returns the retained value, including the
original `reserved_at_ms`. A changed plan/profile is a conflict rather than a
replacement because dispatch may already be in flight. Acknowledgement and
reconciliation check the reservation before recording a workflow, closing the
crash window that previously allowed a second run identity.

## Non-claims

A reservation does not prove dispatch occurred, grant provider authority,
execute a worker, or create a candidate. The ledger does not import Lattice
types; the caller supplies the provider profile and plan identity.
