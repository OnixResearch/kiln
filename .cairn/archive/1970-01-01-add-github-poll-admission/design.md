## Decisions

The pass composes existing bounded normalization, authenticated transport and
the work ledger rather than adding a scheduler. It removes page records from the
transport's view only through the poll loop; the ledger resolves duplicates and
version ordering.

Closed observations are counted separately because closing an issue is not
proof of implementation, cancellation or landing. No cancellation side effect is
invented in this pass.

## Non-claims

A pass is not a daemon, retry policy, claim, dispatch or publication. It does not
prove GitHub visibility beyond the admitted token or that polled text is
execution permission.
