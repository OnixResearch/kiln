## Context

`AttemptLedger` (`crates/kiln-core/src/implementation/attempt.rs`) owns attempts, fences, dispatch reservations and the workflow run each attempt records. Worker results reach it through fenced calls: `start_execution` (acknowledgement), `reconcile_dispatch` (lost acknowledgement), `admit_candidate_handoff`, `fail`, `block`, `cancel` and `reconcile_unknown`. Every call authorizes the exact attempt identity and current fence, so a predecessor cannot act on its own revoked attempt. The run identity it reports is not checked against other attempts.

## Goals and Success Contract

A provider execution identity maps to one attempt for the life of the ledger, and the race matrix of #45 is answered case by case with deterministic fixtures.

## Decisions

### Decision: refuse a run already recorded on another attempt

**Choice:** `start_execution` and `reconcile_dispatch` refuse, with `implementation_dispatch_crossed` and without mutation, an observed workflow run equal to the run recorded on any other attempt of the ledger. The check runs after fence and phase authorization, so a stale caller still sees its stale-fence refusal first.

**Rationale:** the run identity is the provider's execution identity. Two attempts sharing one would let a predecessor's execution, output or termination be read as the successor's. Refusing keeps the successor's dispatch unacknowledged, which the existing unknown-dispatch path already reconciles.

**Alternative:** compare only within one canonical work key. A run reused across different work is equally crossed, so the ledger-wide check is simpler and stricter.

### Decision: fixtures at the pure ledger, restored between callbacks

**Choice:** the new fixtures drive the pure ledger and round-trip it through its serialized form between callbacks, as the existing restart fixture does. The durable store's restart behavior is already covered by `crates/kiln-implementer/tests/durable.rs`.

**Rationale:** the ledger is the authority for every transition; serialization is exactly what the durable store persists.

## Risks / Trade-offs

- The check scans the retained attempts on each acknowledgement. Attempts per ledger are bounded by work budgets, so the scan stays small.
- A provider that legitimately reuses run identities across executions would now fail closed into the unknown-dispatch path. No such provider is admitted.
