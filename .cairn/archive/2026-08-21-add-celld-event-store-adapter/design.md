# Design: celld event-store adapter

## Context

Kiln owns journal meaning, state transitions, outbox semantics, leases, replay, fairness, and effect admission. A storage provider can only persist approved records.

celld `v0.2.1` runs one application deployment on a fleet. Each named Durable Object owns one SQLite database and one serialized execution context.

celld fences object ownership through conditional bucket writes. Its output gate holds a write response until replication and current ownership are checked.

These properties fit one Kiln journal partition. They do not supply a distributed transaction across partitions or a global Kiln scheduler.

## Goal and Success Contract

The goal is an optional `EventStorePort` adapter backed by a versioned Kiln Worker on one exact celld cohort.

Completion requires these results:

- Kiln core and application contracts contain no celld or Worker runtime types.
- One typed profile admits the celld binary, Worker deployment, storage cohort, transport, authentication, and named bounds.
- One deterministic mapping assigns each journal partition to one Durable Object.
- Each accepted event batch and its linked outbox records commit in one SQLite transaction.
- Every response preserves Kiln, request, deployment, cell, and storage identities as separate facts.
- Replay checks canonical bytes, identities, versions, revisions, and bounds before state evolution.
- Ambiguous transport outcomes remain unknown until the same operation identity is reconciled.
- Positive and negative tests cover core, client, Worker, and exact-cohort behavior.

False completion includes these results:

- using the celld internal operator API as the event-store protocol.
- writing private fleet-bucket keys directly.
- enabling `CELLD_OUTPUT_GATE=0` or disabling the storage probe.
- storing events and linked effects in separate transactions.
- retrying an ambiguous append with a new operation identity.
- assuming a Worker deployment reached every node without cohort evidence.
- combining independent cells into an invented global transaction.
- claiming that a celld acknowledgement proves external effect execution.

## Research Sources

The design uses the following sources at revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da`:

- `README.md` for the runtime, bucket, deployment, and fleet model.
- `docs/cloudflare-compat.md` for Durable Object SQLite, Worker API, and missing workflow support.
- `docs/fencing.md` for ownership epochs, output gating, storage requirements, and startup probes.
- `docs/security.md` for listener, authentication, bucket-authority, and alpha operator API boundaries.
- `docs/limitations.md` for one-application fleets, platform limits, and missing scheduler behavior.
- `docs/testing.md` for differential, simulation, crash, takeover, and live-fleet evidence.
- `crates/celld/js/harness.js` for `transactionSync`, serialized storage access, and Durable Object identity behavior.
- `crates/celld/storage.rs` for SQLite transactions, rollback, write positions, and application SQL bounds.
- `crates/celld/deploy.rs` and `crates/celld/protocol.rs` for deployment identity and pointer publication.

## Approach Registry

| Family | Mechanism | Claim | Evidence | State |
|---|---|---|---|---|
| Durable Object event store | Put one journal partition and linked outbox in one object database | Supplies bounded atomic persistence for one partition | SQLite transactions, serialized object access, fencing, and output gating | selected |
| Workflow or build adapter | Run CI tasks through celld | Claims execution effects | Workflows are planned, and process APIs are unavailable | rejected |
| Operator-control adapter | Use `/state`, `/do/NAME`, and `/shutdown` | Claims fleet operations | The API is alpha, internal, and unauthenticated | rejected |
| Direct bucket adapter | Write deployment or cell objects from Kiln | Bypasses the Worker | Private formats, fencing rules, and bucket administrator authority | rejected |
| Native celld library link | Import celld runtime crates into Kiln | Calls runtime internals directly | V8, object-store, and provider coupling cross the core boundary | rejected |

The selected route is the weakest sufficient mechanism. It adds only the storage capability that Kiln already owns as a port.

## Decisions

### Decision: Implement only `EventStorePort`

The adapter implements the application-owned event-store capability. It does not implement `WorkflowPort`, `BuildPort`, or a fleet-management port.

The application shell selects the adapter at a visible composition root. An unavailable celld profile does not cause fallback to another store.

### Decision: Use a purpose-built Worker contract

A stateless Worker receives bounded authenticated requests. It resolves the target Durable Object from an admitted partition name.

The Durable Object implements append, read, claim, complete, reconcile, snapshot, schema, and health operations. Each operation uses one versioned request envelope.

The normal adapter never calls celld internal routes. Deployment and fleet operations remain separate operator actions.

### Decision: Keep storage decisions pure

Pure Kiln functions admit append batches, expected revisions, event identities, outbox linkage, replay windows, lease plans, and migration edges.

The Rust client owns HTTP, TLS, credentials, deadlines, cancellation, retries, and response decoding. The Worker owns request authentication, SQLite, and runtime errors.

The Worker applies an admitted plan. It does not decide canonical CI outcomes, fairness, retries, or provider effect meaning.

### Decision: Partition by explicit authority stream

A profile declares the partition projection and its version. Kiln computes a BLAKE3 partition identity from the admitted stream facts.

The Worker maps that bounded name to one Durable Object. Provider-native object identifiers remain external references and never replace Kiln stream identities.

One append can affect only one partition. Cross-partition reads are projections, and cross-partition transactions are unsupported.

Global fairness and dispatch order remain in the Kiln shell over explicit snapshots. Cell placement or activation order has no scheduling authority.

### Decision: Commit events and outbox records together

The Worker checks the operation identity, request payload identity, stream identity, expected revision, batch bounds, and duplicate identities.

It uses one Durable Object SQLite transaction for the event rows, stream revision, outbox rows, and idempotency result.

A rejected check writes no canonical row. A transaction error returns no successful append result.

### Decision: Require durable-runtime admission

The profile pins celld `v0.2.1`, revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da`, platform asset identity, and Apache-2.0 provenance.

The profile also binds the Worker protocol, deployment SHA-256, compatibility date, storage scheme, bucket identity, and ingress origin.

Runtime admission requires enabled output gating and a successful conditional-write storage probe. Missing observations make the adapter unavailable.

GitHub and celld SHA-256 values remain native interoperability facts. Kiln computes BLAKE3 identities for its own profile and artifact records.

### Decision: Keep authentication outside celld authority

celld does not authenticate public Worker users or terminate TLS. The profile therefore requires an exact trusted ingress and an application authentication policy.

The initial Worker contract uses HMAC-SHA256 because the admitted Worker Web Crypto API supports that algorithm. Secrets remain shell-owned references.

Runtime injection supplies the Worker secret. The generated Wrangler configuration and deployment manifest must not contain secret bytes.

The Worker checks the method, path, timestamp window, nonce, body digest, key identity, and signature before it resolves a cell.

### Decision: Make retries idempotent and ambiguity explicit

Each mutation binds one operation identity and one BLAKE3 payload identity. The Durable Object stores the terminal response with that operation.

A repeated operation with equal payload returns the stored response. A repeated operation with different payload fails as an identity conflict.

A timeout or disconnect after possible acceptance becomes `Unknown`. Reconciliation reads the exact operation before any redispatch.

### Decision: Validate replay at both boundaries

The Worker returns bounded ordered canonical rows with schema, sequence, revision, and stored identity facts.

The Rust client checks framing, count, byte, and pagination bounds. The Kiln core checks canonical identities and folds events through production `evolve`.

A snapshot remains a rebuildable acceleration artifact. It cannot replace missing canonical events or bypass replay compatibility.

### Decision: Keep schema migration explicit

The Worker database has an application schema version that is separate from the celld database format and deployment version.

A pure migration plan names each admitted version edge and expected input identity. The Worker applies one plan in one SQLite transaction.

Unknown, partial, destructive, or cross-object migrations fail closed. Fleet rollout must keep protocol and schema cohorts compatible during the change.

### Decision: Preserve celld security limits

The initial profile permits one declared trust domain. It does not claim hostile tenant isolation on the celld alpha release.

The fleet bucket remains celld administrator authority. Kiln receives no bucket credential unless a separate deployment shell explicitly requires it.

The internal listener stays outside the adapter transport. A private network or encrypted overlay protects that listener under celld policy.

## Data Flow

```text
Kiln command
  -> pure decision and event/outbox plan
  -> application-owned EventStorePort request
  -> bounded celld HTTP client
  -> trusted ingress and authenticated Worker route
  -> named Durable Object
  -> one SQLite transaction
  -> celld output gate and fenced replication
  -> versioned provider observation
  -> pure Kiln admission or reconciliation
```

## Configuration

Typed Nickel contracts define:

- celld release, revision, platform asset, license, and attestation facts.
- Worker protocol, deployment, compatibility date, schema, and migration cohort.
- storage scheme, bucket identity, probe evidence, and output-gate policy.
- ingress origin, TLS, authentication, key reference, nonce, and replay policy.
- partition projection and permitted trust domain.
- request, response, event, outbox, page, byte, diagnostic, and concurrency bounds.
- deadlines, cancellation, definite retry, unknown outcome, and reconciliation policy.
- snapshot, retention, lease, backup-export, and damage-response policy.
- evidence scope and non-claims.

Unknown fields, zero invalid limits, contradictions, drift, or missing observations fail before dispatch.

## Validation

Pure tests cover append admission, partition identity, revision checks, event-outbox linkage, replay, lease plans, and migrations.

Worker tests cover valid transactions, rollback, duplicate operations, payload conflicts, malformed input, authentication, bounds, and schema drift.

Client tests cover exact origins, TLS policy, redirects, timeouts, cancellation, truncation, malformed responses, and unknown outcomes.

Exact-cohort tests cover deployment, restart, node loss, ownership takeover, bucket throttling, storage-probe refusal, and output-gate refusal.

Differential fixtures run the Worker storage contract on the admitted celld cohort and a compatible workerd cohort where the APIs overlap.

Negative fixtures must fail at the intended boundary. An unrelated network or provider error cannot count as rejection evidence.

## Risks and Trade-offs

One hot partition has one writer. Partition policy must match required ordering and measured throughput.

Cold activation adds object-store latency. Kiln must not infer a timeout or failed commit from this delay.

A celld fleet runs one application deployment. Independent Kiln deployments need separate fleet or prefix authority.

The alpha release can change its operator and runtime surfaces. Exact cohort admission makes this drift visible.

Durable Object storage does not provide a global transaction. Global projections and fairness use explicit snapshots and Kiln policy.

## Claim Boundary

Passing checks establishes bounded request admission, one-partition transaction behavior, replay checks, and observed celld compatibility for the pinned cohort.

It does not prove storage hardware correctness, object-store correctness, hostile tenant isolation, global serializability, exactly-once effects, CI success, or release eligibility.
