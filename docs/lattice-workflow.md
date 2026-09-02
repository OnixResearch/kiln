# Lattice workflow adapter

`kiln-adapter-lattice` connects Kiln workflow effects to one pinned Lattice contract.

## Pinned contract

- Repository: `ssh://git@github.com/OnixResearch/lattice.git`
- Revision: `70496e67c7fd4a8b05914161a8e09de2759bebc8`
- Crates: `lattice-workflow-contract` and `lattice-workflow-transport`
- License: `MIT`
- Request: `lattice.workflow-request.v2`
- Observation: `lattice.workflow-observation.v1`
- Graph: `lattice.workflow-graph.v1`

Cargo uses the committed immutable import at
`vendor/lattice/crates/lattice-workflow-contract`, including its `transport`
package, from this exact revision. The repository URL above is provenance, not
an acquisition requirement. See [source acquisition](reference-pins.md#build-source-acquisition)
for the archive BLAKE3 pin and drift-refusing importer. No ambient sibling
checkout or SSH credential is needed to build Kiln.

The producer fixtures are under `fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/`.

The checked-in import also contains the original producer fixtures. Reproducing
the import checks their exact bytes without running the producer. For an
intentional fixture update, supply an explicitly reviewed Git checkout at the
pinned revision (not a build prerequisite):

```console
scripts/sync-lattice-workflow-fixtures.rs "$LATTICE_SOURCE"
scripts/sync-lattice-workflow-fixtures.rs "$LATTICE_SOURCE" --check
```

## Setup

Review `config/lattice-workflow-profile.ncl`. The composition root can construct `UnixWorkflowExchange` with the explicit socket path, peer IDs, deadlines, frame limits, and record bound.

The profile fixes platform, graph schema, node kinds, effect classes, adapter kinds, controls, byte limits, concurrency, and non-claims.

The adapter receives no ambient endpoint, credential, clock, filesystem, or process authority. It never starts a Lattice process.

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

Optional source context binds provider, repository, and exact revision into the
v2 request identity. A reviewed `shell_environment` handler route injects only
the fixed `LATTICE_SOURCE_PROVIDER`, `LATTICE_SOURCE_REPOSITORY`, and
`LATTICE_SOURCE_REVISION` keys into exact mapped shell nodes; source context
itself does not fetch or materialize source. The GitHub source helper and
`route_exact` setup are described in the [operator guide](operator.md#github-ci-adapter).

Equal admitted plans produce equal Lattice requests. A changed graph, profile, capability, or identity requires a new request.

## Lifecycle and recovery

`Accepted`, `Queued`, `Running`, and `Paused` map to pending Kiln state. They cannot report terminal success.

If a dispatch write completes without a valid response, the adapter sends `RecoverDispatch` with the original admitted request. It does not dispatch again.

A recovered exact acceptance enters normal observation. Not-found or another disconnect remains `Unknown` without retry authority.

Reconciliation uses the stored request and accepted execution. A stale execution, earlier sequence, or conflicting duplicate sequence fails without replacing active state.

Cancel, pause, and resume bind the current execution and sequence. Unsupported controls fail before transport.

CI routes require the producer's `lattice.workflow_interchange.shell_success`
semantics: zero shell exit for successful completion, failed nodes for nonzero
or missing successful status, and skipped dependent effects without execution
receipts. The imported contract is not the runtime implementation. Operators
must select a Lattice runtime implementing that requirement; Kiln does not infer
command success from output text or reinterpret an older producer's false
successful observation.

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
