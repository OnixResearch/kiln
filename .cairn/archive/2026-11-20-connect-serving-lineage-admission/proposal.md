## Why

Issue #61 is open after `connect-fair-admission` landed bounded ready-work
admission, held owner debits and parent quota transfer. Three parts of the issue
remain without a deterministic contract:

- Serving backpressure has no input. A worker request only carries the owner's
  declared aggregate `ServingBounds`; Kiln cannot wait before claiming while the
  shared serving owner is saturated, stale, draining or unobserved.
- Recursive lineage (the OASYS RLM addendum) is not closed. Cancelling or
  expiring a parent does not stop descendant dispatch, there is no budget
  revocation, and there is no lineage-wide view that keeps reserved, held,
  unknown, settled and rejected usage apart.
- The policy has no committed generated export that the Rust admission path
  reads back, and recursive budget profiles are not authored in typed Nickel.

## What Changes

- Add a provider-neutral `ServingObservation` (the Animus #17 availability
  snapshot shape: source generation/sequence, deployment generation, readiness,
  origin, freshness window, owner aggregate bounds, remaining slots and queue
  depth, unknown never zero) and an opt-in `serving` policy block. Under it,
  worker-request work waits before claim and before a request when the owner
  scope is saturated (`overloaded`), not ready, stale or missing. A snapshot is
  evidence only: it never reserves, never substitutes for the owner grant and
  never refunds.
- Add lineage revocation, ancestor cancellation/expiry blockers and a
  conservation-checked `LineageUsage` summary over a root's whole subtree.
- Extend `AdmissionExplanation` with lineage and serving facts.
- Add `observe_serving`, `revoke` and `lineage` shell commands over the
  existing implementation store transaction.
- Add Nickel serving-policy and recursive-lineage profile contracts with
  positive fixtures, committed generated JSON exports read by Rust tests, and
  negative fixtures.

## Impact

- **Files**: `crates/kiln-core/src/fair_admission.rs` and new
  `fair_admission/{serving,lineage}.rs` modules plus tests,
  `crates/kiln-core/src/inventory.rs` (one reason label),
  `crates/kiln-implementer/src/admission.rs`, a new
  `crates/kiln-implementer/tests/admission_lineage.rs`, policy literals in
  existing tests/fixture binary, `config/fair-admission-*.ncl`,
  `config/fixtures/*fair-admission*`, `fixtures/fair-admission/*.json`,
  `scripts/check-nickel-fixtures.sh`, `docs/fair-admission.md`, `CHANGELOG.md`.
- **Owner**: Kiln core/implementer maintainers. Consumers: Lattice #24/#25/#6
  through the existing implementer admission commands.
- **Testing**: workspace fmt, clippy, tests, Nickel fixture check, Cairn
  validate/gates/traceability, Octet check.
- **Non-claims**: no live serving, physical placement, provider capacity,
  cross-host or production qualification (#49). No Animus dependency, inference
  router, model eviction or preemption. Defaults are unchanged without the new
  opt-in policy block.
