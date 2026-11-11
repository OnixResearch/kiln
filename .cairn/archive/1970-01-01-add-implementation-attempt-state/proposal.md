## Why

Multiple issue events, Cairn discovery runs, or workers can select the same
implementation work. Lattice's durable execution of one workflow does not
provide a global claim over that work. Kiln must own the work-to-attempt
transition, fence and budget while Lattice owns execution.

Issue: [#45](https://github.com/OnixResearch/kiln/issues/45).

## What Changes

- Add a pure `AttemptLedger` state machine over `ImplementationWork`.
- Claim at most one attempt per canonical work unit, freezing work identity,
  revision, ordinal and the reviewed attempt budget.
- Assign a monotonic fence; every candidate and later request must carry it.
- Add bounded leases, renewal, expiry and replacement with fence revocation.
- Persist claimed, executing, candidate-produced, verifying, blocked, failed,
  cancelled and superseded states with structured causes and distinct failure
  classes.
- Reconcile an ambiguous dispatch against exactly one observed workflow run
  without blind redispatch.
- Apply `AbortActive` or `FinishActive` supersession without mutating a frozen
  attempt contract.
- Serialize the ledger so consumed budget and attempt lineage survive restart.

## Impact

- **Files**: `crates/kiln-core/src/implementation/{mod,attempt}.rs`,
  `docs/implementation-work.md`.
- **Testing**: racing claims, stale fence after expiry, idempotent and
  conflicting dispatch reconciliation, budget/lineage across serialization,
  cancellation, and supersession rules.
- **Authority**: Kiln owns claims, fences, budgets and disposition. This change
  does not add a worker, workflow engine, Lattice route, candidate acceptance,
  publication, or merge authority.
