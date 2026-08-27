# Design: Aspen runtime backend

## Context

Kiln owns a deterministic CI state machine and narrow provider ports. Its current production-oriented Radicle binary constructs a Lattice workflow adapter and a local SQLite replay store.

Aspen provides workload-neutral fabric mechanisms. Its accepted boundaries include system-extension callbacks, durable state, content storage, transport, scheduling, supervision, policy, resources, and evidence.

The integration must compose these systems without making Aspen the owner of CI meaning. It must also avoid direct provider effects inside a hosted callback process.

## Success Contract

Completion requires one hosted Kiln service that can accept a trigger, commit Kiln events and outbox records, dispatch approved provider effects, recover after restart, and publish bounded status.

The same Kiln command and event evolution code must run in standalone and Aspen profiles. A provider observation must pass Kiln admission before it changes CI state.

False completion includes profile-only configuration, direct Aspen imports in core crates, ambient callback effects, split event and outbox commits, hidden provider fallback, or success without linked receipts.

## Decisions

### Decision: Kiln owns the extension protocol

**Choice:** Add a narrow Kiln-owned contract for hosted service requests, callbacks, state snapshots, effect plans, completions, and status.

The contract uses canonical Preserves values and BLAKE3 identities. Aspen runtime handles and Rust layouts never enter the wire contract.

**Rationale:** The application owns the service contract. Aspen supplies mechanisms and must not define CI semantics.

### Decision: Aspen remains a runtime provider

**Choice:** Use Aspen for service hosting, durable state, transport, content, scheduling, supervision, and bounded effect execution.

Lattice remains the selected `WorkflowPort`. Mantle remains the selected `BuildPort`. Cairn remains the selected `GatePort`.

**Rationale:** Replacing those providers would broaden this change and transfer unrelated semantics.

### Decision: Keep Aspen outside Kiln core crates

**Choice:** Place all Aspen dependencies in new adapter and callback-shell crates.

Pure mapping functions receive Kiln values and normalized Aspen contract values. They perform no file, network, process, clock, environment, or credential effects.

**Rationale:** Kiln core independence is an accepted compatibility boundary.

### Decision: Commit events and outbox records atomically

**Choice:** Map one Kiln stream partition to one Aspen durable namespace and atomicity domain.

One append operation checks the expected stream revision and writes canonical journal and outbox rows in one atomic batch. The adapter returns the committed revision only after durable acceptance.

A failure after submission and before a definitive commit result returns `Unknown`. Reconciliation reads the exact operation and stream identities before Kiln decides retry safety.

**Rationale:** A split commit can lose work or dispatch an effect without its accepted event.

### Decision: Use explicit typed fabric effects

**Choice:** The callback executable returns effect requests. It never opens files, sockets, databases, clocks, or child processes.

The initial mappings are:

| Kiln need | Aspen mechanism |
| --- | --- |
| Journal and outbox | durable ordered-state port |
| Provider command | bounded execution port |
| Lattice exchange | transport-session port |
| Source, input, output, and logs | content-store port |
| Timers, deadlines, and leases | time and scheduling ports |
| Operational facts | evidence and observation ports |

Each request binds the active service generation, Kiln effect identity, provider profile, operation identity, resource profile, authority refs, and idempotency key.

**Rationale:** Typed effects preserve authority and simulation boundaries.

### Decision: Use a bounded native callback executable

**Choice:** Publish a callback executable for Aspen's native-process extension profile.

Each invocation receives bounded canonical bytes on standard input. It uses a cleared environment and emits one bounded canonical response on standard output.

The executable performs deterministic command handling and event evolution. It returns state, requested effects, diagnostics, and checkpoint material without performing those effects.

**Rationale:** One-shot callbacks fit the reviewed Bounded Exec mechanism and avoid a hidden long-lived child protocol.

### Decision: Preserve both identity domains

**Choice:** Retain Kiln canonical identities and Aspen artifact, operation, execution, receipt, and content references as distinct typed values.

Equal BLAKE3 text does not prove equal schema, role, namespace, authority, or meaning. Mapping receipts bind both sides explicitly.

**Rationale:** Both systems use BLAKE3, but hash equality alone does not merge semantic domains.

### Decision: Add explicit trigger ingress

**Choice:** Reuse Kiln's Radicle trigger parsing and exact source acquisition. Send the admitted trigger to the hosted service through a bounded Aspen client.

The client binds the repository, exact revision, trigger identity, service generation, protocol profile, authority, and idempotency key. It does not call Lattice directly.

**Rationale:** Hosted Kiln must own command ordering and durable acceptance before provider dispatch.

### Decision: Fail closed without fallback

**Choice:** The Aspen profile names exact contract versions, service and extension identities, fabric ports, execution profile, resource limits, and transport cohort.

Unavailable, stale, incompatible, or over-authorizing bindings fail before dispatch. The shell never falls back to SQLite, standalone, or Lattice-direct operation without a new explicit selection.

**Rationale:** Hidden fallback changes durability, authority, and failure semantics.

### Decision: Stage evidence by claim level

**Choice:** Validate deterministic mapping first, then simulated fabric behavior, separate native callback execution, local live service operation, and restart recovery.

Production readiness remains denied until a later release gate accepts its own deployment, security, reliability, and operating evidence.

**Rationale:** Local conformance cannot prove a production distributed service.

## Required Flow

```text
Radicle or operator trigger
  -> Kiln source and trigger admission
  -> Aspen service request
  -> Kiln pure command decision
  -> atomic journal and outbox commit
  -> admitted typed fabric effects
  -> Lattice, Mantle, Cairn, or other selected provider
  -> typed provider observation
  -> Kiln observation admission
  -> atomic completion events
  -> bounded status and evidence
```

## Test Design

Pure tests cover exact lowering, stable identities, canonical ordering, expected revisions, unsupported capabilities, stale generations, and false success.

Adapter tests cover atomic append, load, effect routing, content mapping, transport acknowledgement, cancellation, timeout, output bounds, and uncertain outcomes.

Callback tests cover valid requests, malformed bytes, missing state, oversized input, denied authority, effect-free denial, deterministic repeat, and checkpoint recovery.

End-to-end tests cover one accepted trigger, one rejected trigger, restart after committed intent, disconnect after acceptance, reconciliation, and unavailable-provider denial.

Architecture tests reject Aspen imports in core crates, ambient callback effects, raw provider handles, unpinned dependencies, and hidden fallback branches.

## Risks and Trade-offs

- A native callback process adds startup cost. The initial profile favors a reviewable boundary over throughput.
- Provider adapters currently own direct process or socket shells. Their Aspen mappings require typed effect plans instead.
- Cross-repository contract drift can break integration. Immutable pins and independent consumer fixtures make drift explicit.
- Aspen durability can preserve bytes without preserving Kiln meaning. Kiln replay remains authoritative for CI state.
- A transport timeout can leave service acceptance unknown. Reconciliation must precede any retry classified as safe.

## Claim Boundary

Passing this change proves bounded compatibility between one Kiln cohort and one Aspen cohort for the tested profiles.

It does not prove Aspen durability, provider correctness, process sandboxing, network delivery, release eligibility, or production availability.
