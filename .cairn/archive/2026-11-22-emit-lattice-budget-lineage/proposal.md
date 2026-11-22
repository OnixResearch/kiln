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
  attempt's remainder. Every required owner must have granted, and every
  dimension Lattice charges that the account tracks must be held; an
  uncharged dimension the attempt does not hold is omitted.
- `--worker-request`: append the lineage as the request's `lineage` member
  only when the attempt holds a committed reservation. Every admitted byte
  stays in place. Without admission the request keeps its bytes and
  identities. The sent bytes are frozen atomically under their own BLAKE3,
  and a separate `transport_requests` record, written with the transport
  intent and launch, keeps that file's path and digest while the launch
  record keeps its published shape. Recovery and campaign export read
  exactly the recorded file; recovery checks that it is the admitted request
  plus at most one well-formed lineage.
- `--dispatch-plan` workflow exchange: derive and check the lineage the same
  way, but leave the `lattice.workflow-request` bytes unchanged, because that
  Lattice contract has no lineage field.
- Refuse with a typed `LineageRefusal` before any transport start when the
  lineage cannot be derived consistently. This covers: an unsettled or
  crossed reservation, a missing required grant, an inconsistent ledger, a
  blocker, backpressure, an exhausted charged dimension, an identity Lattice
  cannot read, or a request that already names a lineage.
- The qualification crash barrier finds Lattice's intent by the digest of
  the sent bytes and retains them as each Lattice stage's request evidence.

## Impact

- **Files**: `crates/kiln-adapter-lattice/src/{lib.rs,lineage.rs,lineage_tests.rs,worker.rs,worker_binding_tests.rs,worker_preparation_tests.rs,bin/lattice-dispatch.rs}`,
  `crates/kiln-core/src/implementation/{attempt.rs,dispatch.rs}`,
  `crates/kiln-implementer/tests/durable.rs`,
  `crates/kiln-web-service/src/implementation_campaign.rs`,
  `scripts/{implementation-qualification.py,test_implementation_qualification.py}`,
  `fixtures/lattice/kiln-budget-lineage/**`, `docs/fair-admission.md`,
  `docs/implementation-runbook.md`, `docs/implementation-qualification.md`,
  `.cairn/specs/fair-admission/spec.md`, `CHANGELOG.md`, this change.
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
