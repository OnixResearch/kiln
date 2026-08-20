# Lattice workflow adapter

`kiln-adapter-lattice` connects Kiln workflow effects to one pinned Lattice contract.

## Pinned contract

- Repository: `ssh://git@github.com/OnixResearch/lattice.git`
- Revision: `66fe7b9b43567490caacc797ebda307e734c4419`
- Crate: `lattice-workflow-contract`
- License: `MIT`
- Request: `lattice.workflow-request.v1`
- Observation: `lattice.workflow-observation.v1`
- Graph: `lattice.workflow-graph.v1`

Cargo uses this exact Git revision. It does not use an ambient sibling checkout.

The producer fixture is under `fixtures/lattice/workflow-contract/66fe7b9b43567490caacc797ebda307e734c4419/`.

Refresh and check it with:

```console
scripts/sync-lattice-workflow-fixtures.rs ../lattice
scripts/sync-lattice-workflow-fixtures.rs ../lattice --check
```

## Setup

Review `config/lattice-workflow-profile.ncl`. Select an application-owned `WorkflowExchangePort` at the composition root.

The profile fixes platform, graph schema, node kinds, effect classes, adapter kinds, controls, byte limits, concurrency, and non-claims.

The adapter receives no ambient endpoint, credential, clock, filesystem, or process authority.

## Dispatch flow

```text
Kiln WorkflowEffectPlan
  -> pure Kiln graph and linkage validation
  -> deterministic Lattice request lowering
  -> Lattice contract admission
  -> application-owned WorkflowExchangePort
  -> accepted native workflow and execution identities
  -> Lattice observation admission
  -> Kiln workflow outcome admission
```

The request binds the Kiln effect, attempt, candidate, pipeline revision, workflow policy, provider profile, graph, and idempotency identity.

Equal admitted plans produce equal Lattice requests. A changed graph, profile, capability, or identity requires a new request.

## Lifecycle and recovery

`Accepted`, `Queued`, `Running`, and `Paused` map to pending Kiln state. They cannot report terminal success.

A transport disconnect after acceptance preserves the exact native execution identity and returns `Unknown`.

Reconciliation uses the stored request and accepted execution. A stale execution, earlier sequence, or conflicting duplicate sequence fails without replacing active state.

Cancel, pause, and resume bind the current execution and sequence. Unsupported controls fail before transport.

## Logs

Log reads bind one execution, node, stream, cursor, and byte bound.

The returned bytes must match one admitted Lattice log reference. Gaps, truncation, retention, and cursor values remain explicit.

## Non-claims

Passing adapter checks does not prove:

- workflow correctness;
- host confinement;
- external effect success;
- credential freshness;
- provider availability;
- release readiness.

Lattice state remains a provider observation. It never replaces canonical Kiln CI state or authority.
