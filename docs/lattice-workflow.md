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

The profile fixes platform, graph schema, node kinds, effect classes, adapter kinds, controls, byte limits, concurrency, and non-claims. The pinned profile admits ordered `build`, `review`, and `test` node kinds with `Build`, `Test`, and `Custom` effect classes; a review node is observation-only and grants no acceptance authority.

The `local` and `remote` adapter labels admit capabilities and bind the exact
reviewed graph; a label does not select placement. Remote whole-workflow execution
uses the existing enrolled Iroh proxy/worker bridge, with the same exact native
Lattice route. The live web worker can use a separately provisioned Git clone via
its explicit v2 source binding; see [remote Lattice workers](operator.md#remote-lattice-workers).
No per-node mixed-host placement is implied.

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

The maintained handler cohort admits cancellation only (`cancel = true`,
`pause = false`, `resume = false`). Controls bind the current execution and
sequence. Pause/resume plans and requests fail before transport. The generic
`Paused` observation mapping remains pending for a future separately admitted
producer cohort; it does not advertise pause support for this handler.

CI routes require the producer's `lattice.workflow_interchange.shell_success`
semantics: zero shell exit for successful completion, failed nodes for nonzero
or missing successful status, and skipped dependent effects without execution
receipts. Select external CI runtime `85a38b6a4bceeae4bb2057cf157e21752100f1b5`,
which includes `shell_success` from `6339864d3c4e5de72ba4f0899b11b5d5b76704b6`
and observed cancellation from `813076d7e42d0ddfb65042bee615bc5f12a2b4d3`.
The MIT contract/transport import `70496e67c7fd4a8b05914161a8e09de2759bebc8`
is not the runtime implementation; its schema compatibility does not qualify
its old handler for CI. No AGPL contract import is authorized by this runtime
selection. Kiln does not infer success from output text or reinterpret an older
producer's false successful observation.

## Logs

Log reads bind one execution, node, stream, cursor, and byte bound.

The returned bytes must match one admitted Lattice log reference. Gaps, truncation, retention, and cursor values remain explicit.

## Implementation dispatch

`kiln-lattice-dispatch` binds one operator-supplied Lattice workflow request to a
reserved implementation attempt. The request must carry the reservation plan as
effect and idempotency identity, the exact attempt/fence, expected output,
repository/base, cancel-only controls, and an agent/local/custom graph. Kiln
validates the selected capability profile before transport and records the
accepted execution on the attempt.

After an ambiguous dispatch, `--recover true` uses Lattice `RecoverDispatch`
instead of submitting a new execution. A disconnected acceptance remains
unknown. Transport acceptance is not a candidate, verification, integration or
completion result; the implementation workflow and worker remain Lattice-owned.

An explicit `--recover true` refuses an attempt with no persisted transport
intent before binding the request or contacting Lattice. A repeat invocation
with a retained intent recovers automatically, using the same attempt/fence,
reserved plan and byte-sealed request; it never redispatches. The operator-only
`GET /api/implementation/dispatch-recovery?attempt=HEX&fence=N` reports the
bounded canonical intent, request digest and observed workflow. To enable a
browser recovery action, provide `--dispatch-recovery PROFILE` alongside
`--implementation-work DATABASE`. Author the profile with
[`web-dispatch-recovery.ncl`](../config/web-dispatch-recovery.ncl) and give
the service only the original reviewed socket/profile/transport and frozen
request, bound to the same database and one authenticated web principal.
The POST supplies only the digest reviewed in the GET at the exact attempt
and fence. The owner rechecks authority and the byte seal before
`RecoverDispatch`; an ambiguous response is not permission to start again.

`--native-runtime-evidence PROFILE` optionally enables the read-only
implementation-attempt inspector for campaign-produced frozen requests. Use
[`web-native-runtime-evidence.ncl`](../config/web-native-runtime-evidence.ncl)
to reference the canonical database, protected campaign bound inputs, and an
immutable Lattice worker transport profile whose `implementation-attempt inspect`
produces `lattice.worker-inventory.v3` (reviewed at
[`d401f75f`](https://github.com/OnixResearch/lattice/blob/d401f75f8d3494cead30d085288b4bbe032ce871/docs/worker-serving-binding.md)).
Fleet shows retained lifecycle and a bounded set of per-request model observations.
It separates a response-matched model label from the admitted deployment and
replica, neither of which proves which replica answered or which weights ran.
Unattributed broker traffic and incomplete evidence stay distinct. Retained
worker state does not prove physical host reachability.

Default OMP does not export child lineage. The separate opt-in Animus profile
uses an `animus_implementation` declaration in the native seed rather than
`omp`; native `prepare` must qualify the selected profile. Fleet displays direct
children only from the Lattice readback of `animus.implementation-child-lineage.v1`
for the same sealed request, work, attempt, fence, profile and worker incarnation.
Partial or missing coverage does not mean there were no children.
The host adapter is explicitly injected and revision-bound. The stock Lattice
CLI refuses to start an Animus attempt without that adapter; a seed declaration
does not grant execution, workspace or provider authority.

## Non-claims

Passing adapter checks does not prove:

- workflow correctness;
- host confinement;
- external effect success;
- credential freshness;
- provider availability;
- release readiness.

Lattice state remains a provider observation. It never replaces canonical Kiln CI state or authority.
