# Kiln architecture

Kiln owns CI domain meaning. Provider shells own external effects and native authority.

## Crate boundaries

| Crate | Role |
|---|---|
| `kiln-core` | Pure `no_std + alloc` decisions, events, projections, and validation. |
| `kiln-provider-api` | Narrow versioned provider ports. |
| `kiln-codec` | Packed Preserves encoding and BLAKE3 identity. |
| `kiln-standalone` | Reference journal, outbox, log store, and recovery shell. |
| `kiln-radicle-source-contract` | Pure `no_std + alloc` signed-reference request, observation, snapshot, and receipt admission. |
| `kiln-adapter-jj` | Jujutsu source-provider compatibility. |
| `kiln-adapter-git` | Bounded read-only access to local Git repositories. |
| `kiln-adapter-pijul` | Exact Pijul state observation and bounded local snapshot acquisition. |
| `kiln-adapter-lattice` | Versioned Lattice workflow lowering, lifecycle admission, controls, logs, and reconciliation. |
| `kiln-adapter-cairn` | Exact Cairn gate lowering, bounded execution, result admission, and reconciliation. |
| `kiln-adapter-mantle` | Exact Mantle build lowering, bounded execution, product admission, logs, and reconciliation. |
| `kiln-adapter-nix-eval` | Exact stock Nix job discovery, bounded stream admission, task mapping, and GC-root handoff. |
| `kiln-adapter-remote` | Remote execution compatibility. |
| `kiln-adapter-radicle` | Exact local signed-source acquisition, trigger compatibility, bounded ci-job status publication, and declared single-writer patch reviews. |
| `kiln-adapter-cob` | Profile-driven COB observation mapping. |
| `kiln-adapter-cdevents` | Bounded CDEvents and CloudEvents interchange. |
| `kiln-adapter-opentelemetry` | Rebuildable telemetry projection and bounded OTLP shell. |
| `kiln-adapter-valence` | Recorded-only Valence Evidence IR projection and import. |
| `kiln-adapter-webhook` | Registered-endpoint notification delivery with bounded authority. |
| `kiln-adapter-sqlite` | Durable local event, outbox, lease, snapshot, migration, and backup storage. |
| `kiln-adapter-celld` | Optional pinned Celld Worker event store with authenticated bounded HTTPS, one-object SQLite journal/outbox, and conditional lease/schema plans. |
| `kiln-conformance` | Shared positive and negative provider gates. |
| `kiln-cli` | Local operator shell. |

The core has no filesystem, process, network, clock, credential, or provider-runtime dependency.

`kiln-provider-api` owns the source-repository, Pijul-repository, collaboration-mutation, discovery, workflow, gate, remote-execution, remote-lifecycle, and trigger-intake ports. Concrete adapter crates implement these ports. They do not publish replacement application contracts.

Each port returns typed domain or infrastructure failures. Timeout, cancellation, and unknown results stay distinct until an application shell applies retry policy.

The Radicle source contract has no provider runtime dependencies. The source shell consumes its exact published revision.

Source, status, and mutation shells keep Radicle crates behind adapter-owned boundaries.

SecretSpec and Radicle crates do not enter `kiln-core` or standalone builds.

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

Kiln canonical artifacts use `kiln.packed-preserves.v1`. The codec computes their BLAKE3 identities.

The pure core computes domain-owned BLAKE3 identities for discovered task mappings. It receives all identity inputs explicitly.

Provider references include a namespace, native kind, and native value. Equal provider text does not make two references equal.

## Source and gating

Source discovery, exact resolution, acquisition, and mutation use separate operations. An evaluation keeps its exact resolved revision and tree identity.

A candidate set binds ordered changes, dependencies, target revisions, snapshots, pipeline revisions, and gate policy. A moved target creates a new candidate.

A gate effect binds the candidate root, pipeline revision, policy identity, sorted evidence set, provider profile, effect, and native target. The core admits exact provider observations before it maps pass or block into a candidate result.

A merge result creates only a collaboration effect request. The collaboration provider keeps merge authority.

## Operations

The journal uses expected revisions. The outbox uses effect identities, leases, and idempotency keys.

An expired or ambiguous lease enters reconciliation. Kiln does not treat it as work that never started.

Tenant ordering uses integer cross multiplication. Weighted-scalar and dominant-share strategies reject unsupported dimensions.

## Claim boundary

A passing decision proves the checked relation between one admitted state, command, event set, and effect set.

It does not prove provider behavior, source trust, sandboxing, build correctness, or release eligibility.
