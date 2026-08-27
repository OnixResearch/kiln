# Aspen runtime backend

## Scope

The Aspen backend gives Kiln a workload-neutral runtime substrate. Kiln still owns CI commands, events, candidates, attempts, policy, retries, provider selection, outcomes, and release meaning.

The reviewed runtime cohort is Aspen/Molten revision `7fb4c9de293f815654e1e3b954f4fee3678ac87f`. Its bounded process mechanism is Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

The initial claim level is `local-live-pilot`. It is not a production claim.

## Components

`kiln-adapter-aspen` owns these application adapters:

- exact runtime profile admission;
- distinct Kiln and Aspen identity mappings;
- atomic journal, outbox, and revision mapping for `EventStorePort`;
- bounded source, input, output, log, and checkpoint content mapping;
- exact fabric-effect mapping;
- Lattice workflow exchange through an Aspen transport-session port;
- remote lifecycle mapping;
- Radicle and operator trigger ingress;
- fail-closed runtime composition;
- local durability, content, service, restart, and reconciliation fixtures.

`kiln-aspen-extension` owns the deterministic callback core and the one-shot callback executable. The core reads no files, sockets, process state, environment, database, credentials, or clock.

`LocalAspenService` is a deterministic model fixture. It is not an implementation of Aspen's native host.

Aspen owns only the selected mechanisms. Lattice still owns workflow execution. Mantle still owns builds. Cairn still owns lifecycle-gate evaluation. Valence still owns evidence identity and linkage.

## Configuration

Validate and export the typed profile:

```console
nickel export config/aspen-runtime-profile.ncl
nickel export config/fixtures/aspen-runtime-positive.ncl
```

The profile binds these facts:

- source and Bounded Exec revisions;
- native-host, callback, ingress, and Kiln extension schemas;
- service, extension, endpoint, transport, policy, authority, and resource identities;
- ALPN and packed-Preserves framing;
- durability, content, execution, transport, time, scheduling, and evidence ports;
- callback, content, log, effect, record, and deadline bounds;
- service generation and local claim level;
- all required non-claims;
- `fallback = "none"`.

If one fact drifts or one port is unavailable, Aspen selection fails. It does not fall back to SQLite, standalone execution, or direct Lattice dispatch.

## Request flow

```text
Radicle or operator trigger
  -> Kiln source and trigger admission
  -> versioned Aspen ingress envelope
  -> deterministic Kiln callback
  -> atomic journal and outbox intent
  -> exact typed fabric effects
  -> selected Lattice, Mantle, Cairn, or other provider
  -> typed provider observation
  -> Kiln observation admission
  -> generation-fenced completion
  -> bounded status and evidence
```

The callback emits effect plans only after Kiln admission. It does not execute provider effects.

## Port mappings

| Kiln need | Aspen binding | Kiln-retained meaning |
| --- | --- | --- |
| Journal and outbox | `molten.fabric.durability.ordered-state@v1` | event order, stream revision, effect intent |
| Source and artifacts | `molten.fabric.content-store@v1` | source, input, output, log, and checkpoint roles |
| Provider process | `molten.fabric.execution.bounded-process@v1` | executable authorization, provider policy, outcome |
| Lattice exchange | `molten.fabric.transport.session@v1` | workflow plan and completion admission |
| Deadlines | `molten.fabric.time@v1` | retry and timeout policy |
| Scheduling | `molten.fabric.scheduler@v1` | candidate, fairness, and provider selection |
| Operational facts | `molten.fabric.observation@v1` | CI state and release meaning |

Equal BLAKE3 text does not merge these identity domains. Mapping receipts retain namespace, role, schema, producer effect, byte count, and verification receipt.

## Atomic durability

One Kiln stream maps to one Aspen durable namespace and atomicity domain.

An append performs these steps:

1. Validate the exact stream and expected revision.
2. Validate contiguous journal revisions and duplicate identities.
3. Derive one deterministic operation identity.
4. Submit journal records, outbox records, and the next revision as one batch.
5. Return the revision only after an exact accepted receipt.

A conflicting revision changes nothing. A timeout or cancellation before commit changes nothing. A lost acknowledgement after a possible commit returns `Unknown` with the operation identity. Reconcile that exact operation before any retry decision.

## Callback protocol

The callback uses one bounded packed-Preserves frame on standard input and one bounded frame on standard output. The frame contains a versioned Kiln request or outcome payload.

The callback executable uses a cleared environment in the Aspen execution profile. It performs deterministic state and effect planning only.

The callback rejects these inputs before visible effects:

- malformed or non-canonical Preserves;
- oversized input;
- stale service generation;
- wrong profile, authority, policy, or resource identity;
- substituted trigger identity;
- stale or duplicate effect completion;
- shutdown with unresolved effects.

## Recovery

Persist callback and effect intent before external effects. On restart:

1. Load the canonical state and checkpoint.
2. Reject schema, profile, or generation drift.
3. Rebuild the Kiln state from canonical records.
4. List unresolved effects.
5. Reconcile each exact Aspen operation.
6. Apply a linked provider observation through Kiln admission.
7. Dispatch only an operation that Kiln classifies as safe to dispatch.

A stale completion cannot mutate the active generation. An unknown outcome never becomes success or safe retry by default.

## Operator status

Status includes these bounded facts:

- exact Kiln and Aspen profile identities;
- service generation and state revision;
- run and unresolved-effect counts;
- drain and stop state;
- local claim level;
- required non-claims.

Status does not expose credentials, raw runtime handles, host paths, or provider secrets.

## Validation

Run the focused checks:

```console
nix develop path:$PWD -c cargo test -p kiln-adapter-aspen -p kiln-aspen-extension
nix develop path:$PWD -c cargo clippy -p kiln-adapter-aspen -p kiln-aspen-extension --all-targets -- -D warnings
nix build path:$PWD#checks.x86_64-linux.nickel -L
```

The conformance suite covers exact profile admission, positive and negative callback execution, atomic conflicts, timeout, cancellation, post-accept disconnect, content loss and corruption, restart, replay, stale completion, reconciliation, explicit runtime selection, and a separate callback process.

## Current integration blocker

Aspen revision `7fb4c9de293f815654e1e3b954f4fee3678ac87f` passes callback references, but it does not materialize application bytes for the native process.

The native envelope contains `event_ref`, optional `payload_ref`, and optional `state_ref`. Its outcome contains output, effect, state, and checkpoint references. It does not carry the canonical Kiln command, prior state, effect body, completion body, or next state bytes.

Kiln cannot reconstruct CI commands or replay state from BLAKE3 references alone. Precomputing the decision in the ingress client would move CI authority outside the hosted callback. Treating the local model as Aspen would create false completion.

An Aspen follow-up contract must add bounded, identity-checked value materialization for ingress, prior state, effect completion, effect bodies, and callback state. It must preserve intent-before-effect ordering and use a new exact protocol version without fallback.

Until that contract is accepted, the adapter contracts and simulations are usable for local design validation only. The hosted end-to-end gate remains incomplete.

## Rollout gates

A production rollout remains denied until a later release change accepts all of these items:

- a deployed Aspen endpoint and exact node composition;
- independent source and executable binding evidence;
- durable restart and corruption drills;
- transport disconnect and partition drills;
- provider-process teardown evidence;
- secret and authority review;
- bounded load and soak evidence;
- backup, restore, upgrade, and rollback evidence;
- production observability and operator runbooks;
- release-authorized evidence linkage.

## Non-claims

This backend does not prove:

- CI correctness;
- provider correctness;
- process sandboxing or hermeticity;
- exact-once delivery;
- source trust;
- global durability;
- network delivery;
- production availability;
- release eligibility.

A local passing fixture proves only bounded compatibility for the exact tested Kiln and Aspen cohorts.
