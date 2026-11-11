## Decisions

The loop opens the implementer ledger once and calls the same queue drain used by
the one-shot command, so policy, verification and ledger behavior cannot drift.
Failures stop the loop rather than being retried silently.

Signals set one atomic stop flag checked after each drain. The iteration bound is
useful for bounded cron-style runs and tests; without it, the loop runs until a
signal.

## Non-claims

The loop admits deliveries only. It does not claim work, create leases, dispatch
a worker, verify a candidate or publish.
