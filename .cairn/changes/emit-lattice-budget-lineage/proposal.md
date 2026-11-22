## Why

Lattice `5a8e1d51` (Lattice #6) accepts an optional
`lattice.kiln-budget-lineage.v1` snapshot in `lattice.implementation-attempt-run.v3`
requests. Kiln #61 admits a recursive budget lineage, and the host-owner
admission work roots campaign accounts, but `kiln-lattice-dispatch` never
emits the lineage. Lattice therefore cannot hold a dispatched attempt inside
its account's remainder, and Lattice #6 keeps "Kiln sending lineage" open.

## What Changes

- Derive the lineage from Kiln's admission ledger when a dispatch route
  builds its request: the campaign root, the queued work's account, and the
  attempt's owner-committed reservations, whose sum per dimension is the
  attempt's remainder. Every dimension the account tracks must be held.
- `--worker-request`: append the lineage as the request's `lineage` member
  only when the attempt holds a committed reservation. Every admitted byte
  stays in place. Without admission the request keeps its bytes and
  identities. Recovery reuses the frozen dispatched request after checking
  that it is the admitted request plus one well-formed lineage.
- `--dispatch-plan` workflow exchange: derive and check the lineage the same
  way, but leave the `lattice.workflow-request` bytes unchanged, because that
  Lattice contract has no lineage field.
- Refuse with a typed `LineageRefusal` before any transport start when the
  lineage cannot be derived consistently. This covers: an unsettled or
  crossed reservation, an inconsistent ledger, a blocker, backpressure, an
  exhausted dimension, an identity Lattice cannot read, or a request that
  already names a lineage.
- Campaign candidate export names the dispatched request, the bytes Lattice
  bound the attempt to.

## Impact

- **Files**: `crates/kiln-adapter-lattice/src/{lib.rs,lineage.rs,lineage_tests.rs,worker.rs,bin/lattice-dispatch.rs}`,
  `crates/kiln-web-service/src/implementation_campaign.rs`,
  `fixtures/lattice/kiln-budget-lineage/**`, `docs/fair-admission.md`,
  `docs/implementation-runbook.md`, `CHANGELOG.md`, this change.
- **Owner**: Kiln core/implementer maintainers. Consumer: Lattice
  `implementation-attempt run` at `5a8e1d51`.
- **Testing**: focused `kiln-adapter-lattice` lineage and worker tests, the
  campaign tests, fmt, clippy `-D warnings`, `cairn validate` and the change
  gates.
- **Non-claims**: no live Lattice worker, owner, model or serving capacity is
  exercised. Lattice's acceptance is checked against its published schema as
  data, plus a one-off run of Lattice's own `Binding` code recorded as
  evidence. Lattice-side lineage enforcement applies only to the worker
  route.
