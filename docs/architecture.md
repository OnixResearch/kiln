# Kiln architecture

Kiln owns CI domain meaning. Provider shells own external effects and native authority.

## Crate boundaries

| Crate | Role |
|---|---|
| `kiln-core` | Pure `no_std + alloc` decisions, events, projections, and validation. |
| `kiln-provider-api` | Narrow versioned provider ports. |
| `kiln-codec` | Packed Preserves encoding and BLAKE3 identity. |
| `kiln-observe-core` | Pure `no_std + alloc` bounded, deterministic, read-only observation projection with typed profiles, paging and window admission, redaction, and `kiln.observe.view.v1` identities. |
| `kiln-adapter-http-observe` | Bounded HTTPS shell serving accepted observation views as canonical JSON plus a bounded SSE event leg, with Ed25519 or UCAN holder-invocation auth, TLS, and no control authority. |
| `kiln-adapter-iroh-observe` | Bounded `kiln.observe.v1` iroh shell serving accepted views to allowlisted peers with BLAKE3-verified payloads and HTTP/iroh view equivalence. |
| `kiln-standalone` | Reference journal, outbox, log store, and recovery shell. |
| `kiln-radicle-source-contract` | Pure `no_std + alloc` signed-reference request, observation, snapshot, and receipt admission. |
| `kiln-adapter-jj` | Jujutsu source-provider compatibility. |
| `kiln-adapter-git` | Bounded read-only access to local Git repositories. |
| `kiln-adapter-pijul` | Exact Pijul state observation and bounded local snapshot acquisition. |
| `kiln-adapter-lattice` | Versioned Lattice workflow lowering, lifecycle admission, controls, logs, and reconciliation. |
| `kiln-adapter-iroh` | Authenticated, bounded native Lattice workflow transport through a controller-local Unix proxy and worker-local Lattice socket; no CI-state or execution authority. |
| `kiln-adapter-aspen` | Exact Aspen/Molten runtime profile, identity, atomic durability, content, typed effects, ingress, and bounded Unix transport mapping. |
| `kiln-aspen-extension` | Deterministic Kiln callback decisions and one-shot native callback executable bound to the host's exact runtime profile. |
| `kiln-adapter-cairn` | Exact Cairn gate lowering, bounded execution, result admission, and reconciliation. |
| `kiln-adapter-mantle` | Exact Mantle build lowering, bounded execution, product admission, logs, and reconciliation. |
| `kiln-adapter-nix` | Exact local Nix derivation execution, native realisation admission, rooted products, bounded logs, and reconciliation. |
| `kiln-nix-build` | Discovery-task to exact Nix profile/plan mapping, durable build state, operator CLI, and output GC-root lifecycle. |
| `kiln-adapter-nix-eval` | Explicit exact stock/Onix evaluator cohorts, bounded job discovery, task mapping, and GC-root handoff; no Nix library linkage into Kiln. |
| `kiln-onixos-pilot` | Isolated signed operator initiation, durable canonical journal and replay-backed UCAN observation; composes existing Aspen/Lattice ports without production status writes. |
| `kiln-onixos-worker` | Exact source-bound discovery and rooted Nix build worker; fixed-profile Radicle project acquisition/import and explicit source/output-root release. |
| `kiln-radicle-ci` | Broker push admission, reserved exact signed source, canonical journal/replay, existing Aspen/Lattice execution and separately authorized revision-bound status outbox. |
| `kiln-adapter-remote` | Remote execution compatibility. |
| `kiln-adapter-radicle` | Exact local signed-source acquisition, trigger compatibility, bounded ci-job status publication, and declared single-writer patch reviews. |
| `kiln-adapter-github` | Signed push/PR admission, profile-bound SQLite replay/lifecycle shell, exact HTTPS source job, SecretSpec resolution, and bounded commit-status publication through reviewed Lattice dispatch. |
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

The standalone `runtime/kiln-aspen-host` composition shell installs or recovers the
real Aspen native service, invokes `kiln-aspen-extension` as a bounded process, and
routes admitted typed workflow effects through `kiln-adapter-lattice`. Radicle
intake explicitly selects `--runtime lattice` or `--runtime aspen`; Aspen requires
its exact profile and socket and never falls back to direct Lattice dispatch.
The host reserves ingress and dispatch intent durably, keeps ambiguous effects
unknown, and recovers through Lattice `RecoverDispatch` and `Observe` rather than
normal redispatch. Provider completion evidence remains subject to Kiln outcome
admission. `LocalAspenService` is only a deterministic model fixture, not this
native host. The separate `runtime/kiln-radicle-nix-provider` shell owns bounded
Nix execution and durable reports, not CI or release authority.

The isolated OnixOS pilot composes that maintained host with an independently
owned Lattice workflow. The coordinator commits canonical start intent before
dispatch, admits completion only from matching host and worker observations,
and replays the event journal for HTTPS observation. The worker retains build
roots until durable handoff and explicit release. An operator-signed request is
not a forge webhook; neither the pilot nor its observation server grants
production control, status-publication or deployment authority. See the
[repeatable operator procedure](operator.md#isolated-maintained-cohort-onixos-pilot).

The Radicle source-event composition is distinct from the signed operator pilot.
It pins the configured repository/signer/branch once, persists canonical start
and source intent before native submission, and maps the same reviewed project
worker operation across exact revisions. Worker results bind the materialized
source, immutable store import, discovery task and actual build observation.
Only matching worker and native terminal evidence can complete canonical CI.
Duplicate and restart paths reconcile the original operation; status uses its
own exact-revision intent and authority. Bounded polling retains the original
submit reply, latest progress and distinct terminal decisions rather than
eventually exhausting recovery. See the
[adoption and proof procedure](operator.md#radicle-source-to-build-composition).

The Iroh bridge preserves the existing Lattice exchange contract, peer-pins the
worker, enrolls controllers, and checks the exact native capability profile
before forwarding. Existing consumers select its private proxy socket without
an API or provider-ID cutover. Ambiguous exchanges retain unknown-after-write;
only exact native recovery can resolve them. Lattice still owns execution and
durability. The bridge adds no fallback executor or core runtime dependency.

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

## GitHub effect boundary

`kiln-adapter-github` is a provider shell, not a new workflow runtime or CI-state
authority. HMAC-SHA256 authenticates the bounded original webhook body before
JSON parsing or durable admission. Exact repository/candidate/pipeline facts
bind immutable run identities; delivery IDs alone do not establish freshness.
SQLite reserves dispatch and status writes before external effects, binds the
database to the admitted service profile, and fails closed on record exhaustion.
Recovery uses the original Lattice request and exact status lookup rather than
blind redispatch or repeating an ambiguous POST. Unknown stays distinct from
failure and success. Provider observations are admitted through existing Kiln
workflow and CI transitions; publication cannot override a CI outcome.

The separate `kiln-github-job` binary verifies BLAKE3 of original job-profile
JSON bytes, consumes Lattice's three fixed source-context keys, acquires an exact
GitHub commit via HTTPS, admits its bounded Git snapshot, materializes a temporary
checkout without Git metadata, and runs fixed reviewed argv. Source credentials
are not command environment. The Lattice saved workflow, operation identities,
exact route/revision/node mapping, and `shell_environment` binding remain
operator-reviewed prerequisites. Neither a temporary checkout nor bounded
execution is host confinement; separate worker identity and isolation are
operator responsibilities. Fork execution is denied by the Nickel contract.

Kiln maintainers own adapter compatibility and the local repeatability rail;
operators own HTTPS ingress, live tokens, route installation, replay retention,
and deployment. Test-only local Git and loopback status transports require the
explicit `protocol-fixtures` feature, absent from normal packages. See the
[GitHub operator procedure](operator.md#github-ci-adapter) for real configuration
inputs and the local-evidence claim boundary.

## Claim boundary

A passing decision proves the checked relation between one admitted state, command, event set, and effect set.

It does not prove provider behavior, source trust, sandboxing, build correctness, or release eligibility.
