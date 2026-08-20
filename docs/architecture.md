# Kiln architecture

Kiln owns CI domain meaning. Provider shells own external effects and native authority.

## Crate boundaries

| Crate | Role |
|---|---|
| `kiln-core` | Pure `no_std + alloc` decisions, events, projections, and validation. |
| `kiln-provider-api` | Narrow versioned provider ports. |
| `kiln-codec` | Packed Preserves encoding and BLAKE3 identity. |
| `kiln-standalone` | Reference journal, outbox, log store, and recovery shell. |
| `kiln-adapter-jj` | Jujutsu source-provider compatibility. |
| `kiln-adapter-git` | Bounded read-only access to local Git repositories. |
| `kiln-adapter-lattice` | Versioned Lattice workflow lowering, lifecycle admission, controls, logs, and reconciliation. |
| `kiln-adapter-remote` | Remote execution compatibility. |
| `kiln-adapter-radicle` | Radicle broker protocol compatibility. |
| `kiln-adapter-cob` | Profile-driven COB observation mapping. |
| `kiln-adapter-cdevents` | Bounded CDEvents and CloudEvents interchange. |
| `kiln-adapter-opentelemetry` | Rebuildable telemetry projection and bounded OTLP shell. |
| `kiln-adapter-valence` | Recorded-only Valence Evidence IR projection and import. |
| `kiln-adapter-webhook` | Registered-endpoint notification delivery with bounded authority. |
| `kiln-adapter-sqlite` | Durable local event, outbox, lease, snapshot, migration, and backup storage. |
| `kiln-conformance` | Shared positive and negative provider gates. |
| `kiln-cli` | Local operator shell. |

The core has no filesystem, process, network, clock, credential, or provider-runtime dependency.

`kiln-provider-api` owns the source-repository, collaboration-mutation, workflow-execution, workflow-exchange, remote-execution, remote-lifecycle, and trigger-intake ports. Concrete adapter crates implement these ports. They do not publish replacement application contracts.

Each port returns typed domain or infrastructure failures. Timeout, cancellation, and unknown results stay distinct until an application shell applies retry policy.

The application composition root selects each concrete adapter. The core and adapters do not select fallback providers.

## Production transition

Kiln uses one command decision and one event evolution path:

```text
state + command -> events + requested effects
state + event   -> new state
```

A shell commits accepted events and outbox records together. The shell dispatches effects only after provider capability admission.

Replay folds events through `evolve`. Replay does not return provider effects.

## Identity

Kiln canonical artifacts use `kiln.packed-preserves.v1`. The codec computes their BLAKE3 identities outside the pure core.

Provider references include a namespace, native kind, and native value. Equal provider text does not make two references equal.

## Source and gating

Source discovery, exact resolution, acquisition, and mutation use separate operations. An evaluation keeps its exact resolved revision and tree identity.

A candidate set binds ordered changes, dependencies, target revisions, snapshots, pipeline revisions, and gate policy. A moved target creates a new candidate.

A merge result creates only a collaboration effect request. The collaboration provider keeps merge authority.

## Operations

The journal uses expected revisions. The outbox uses effect identities, leases, and idempotency keys.

An expired or ambiguous lease enters reconciliation. Kiln does not treat it as work that never started.

Tenant ordering uses integer cross multiplication. Weighted-scalar and dominant-share strategies reject unsupported dimensions.

## Claim boundary

A passing decision proves the checked relation between one admitted state, command, event set, and effect set.

It does not prove provider behavior, source trust, sandboxing, build correctness, or release eligibility.
