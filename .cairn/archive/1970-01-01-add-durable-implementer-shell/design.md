## Decisions

Single-row JSON documents keyed by ledger kind keep the pure core types
authoritative and avoid a second schema translation. Each mutation uses
`BEGIN IMMEDIATE`; SQLite serializes writers, so a second scheduler observes the
first claim or waits, then finds no eligible work. WAL mode is left to deployment;
the busy timeout covers short critical sections.

The shell stores both ledgers in the same transaction. A failed closure rolls
back both, so an admitted work version and its claim cannot diverge.

`claim_next` ranks via the pure ledger and tries each ready work unit. A claim
that loses a race or exhausts budget is skipped; other independent work may be
claimed in the same call.

## Non-claims

The shell is not a worker, workflow engine, candidate verifier, publisher, or
authority source. It does not poll providers, receive webhooks, or expose HTTP.
Lattice execution/cancellation and Cairn readiness intake remain prerequisites
for a live implementer service.
