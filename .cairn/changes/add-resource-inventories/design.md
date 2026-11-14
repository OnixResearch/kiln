## Context

The existing workers v1 view is project-slot configuration. Inventory v2 is explicitly separate and opt-in, leaving canonical observation v1 unchanged. Provider identity is always scoped; host labels cannot join independent inventories.

## Goals and Success Contract

Four distinct admitted inventories and deterministic bounded explanations for exact admitted work. Unknown, stale, missing, out-of-order and partial inputs must never establish ready/free. Definition existence does not create a worker. Physical shared pools and deployment allocations are counted once. Display-time eligibility cannot dispatch.

## Decisions

A pure no_std core admits bounded versioned snapshots against a principal-specific provider allowlist and prior-generation ledger. Exact scoped identities join workers, leases, services and pools. Freshness is evaluated at a shell-supplied clock. Rows retain evidence identities and supplied unknowns. Projection emits deterministic BLAKE3 identities after redaction. Readiness checks each independent constraint, returns explicit blockers and exposes candidate truncation.

The provider port only reads supplied inventory and exact work needs. A bounded local-file shell uses explicitly named inputs, no discovery/network calls or writes. The authenticated API and offline JSON/HTML consumers call the same projection. Public opaque identifiers prohibit raw paths/URLs/secrets. Offline HTML escapes all values and embeds no executable data or remote assets. Workers v1 remains separate alongside the new panels.

Revision state and qualification facts are producer assertions, not actions or verified weights. Authoritative admission/rejection facts remain separate from explanation; stale explanations cannot grant dispatch. Live qualification requires external producer implementations and operator-approved two-CPU-worker/shared-remote-service exercise.

## Verification Strategy

Add positive and negative pure tests for schemas, provenance, bounds, out-of-order generations, stale/missing/partial providers, exact joins, shared pools, loading/saturation, unsupported task constraints, request rejection and rollout/evidence states. Node tests exercise inert rendering and malformed snapshots. Main integration owner performs coordinated verification after all slices land. Native proposal/design/tasks gates precede product edits.

## Risks and Trade-offs

An inventory snapshot is not a resource grant, a past probe is not capacity, claimed model identity is not weight verification, and unknown remainder is never zero. Supplied facts require qualified authorized producers.
