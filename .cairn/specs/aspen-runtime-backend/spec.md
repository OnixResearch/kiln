# Aspen Runtime Backend Specification

## Purpose

Defines the `aspen-runtime-backend` capability.

## Requirements

### Requirement: Aspen runtime profiles bind an exact cohort

r[kiln.aspen_runtime.profile] Kiln MUST select Aspen through a typed profile that binds immutable source, contract, extension, execution, transport, durable-state, content, scheduling, resource, authority, evidence, and non-claim identities.

#### Scenario: Exact profile is selected

- GIVEN every required Aspen cohort and fabric-port identity matches the reviewed profile
- WHEN the application composition root selects `aspen-runtime-v1`
- THEN Kiln MUST admit the profile before service activation
- AND the admission result MUST bind the exact profile identity.

#### Scenario: A required port drifts

- GIVEN a supplied port has a different version, schema, implementation profile, or authority set
- WHEN profile admission runs
- THEN Kiln MUST reject the profile before transport, storage, or process effects
- AND it MUST NOT select a fallback runtime.

### Requirement: Kiln retains CI authority

r[kiln.aspen_runtime.boundary] Kiln MUST retain command, event, candidate, attempt, policy, retry, provider-selection, outcome, and release meaning when Aspen supplies runtime mechanisms.

#### Scenario: Aspen reports a provider completion

- GIVEN Aspen returns a structurally valid provider observation
- WHEN Kiln evaluates the observation
- THEN Kiln MUST apply its own provider and CI admission rules
- AND Aspen's success label MUST NOT directly mutate canonical Kiln state.

#### Scenario: Core imports Aspen runtime code

- GIVEN `kiln-core`, `kiln-provider-api`, or `kiln-codec` imports Aspen runtime types
- WHEN dependency-boundary validation runs
- THEN validation MUST fail with the forbidden dependency edge.

### Requirement: The callback contract is canonical and effect-free

r[kiln.aspen_runtime.callback] The Aspen callback executable MUST consume bounded canonical input and emit bounded canonical state, effect-plan, checkpoint, status, or denial output without performing provider effects.

#### Scenario: A valid callback runs

- GIVEN a canonical request, prior Kiln state, active generation, deadline, policy refs, and resource refs
- WHEN Aspen invokes the callback executable
- THEN the executable MUST return the deterministic Kiln decision and requested effects
- AND it MUST NOT open an ambient file, socket, clock, database, environment, credential, or child process.

#### Scenario: Callback input is malformed or oversized

- GIVEN callback bytes are malformed, non-canonical, stale, or over the selected input bound
- WHEN callback admission runs
- THEN the invocation MUST fail before Kiln state changes or effect requests become visible.

### Requirement: Journal and outbox commits are atomic

r[kiln.aspen_runtime.durability] The Aspen `EventStorePort` adapter MUST commit accepted journal records, linked outbox records, and the next stream revision in one durable atomicity domain.

#### Scenario: Expected revision matches

- GIVEN a stream at revision N and an append request expecting revision N
- WHEN Aspen durably commits the journal and outbox batch
- THEN the adapter MUST return the exact next revision
- AND a subsequent load MUST return both journal and outbox records in canonical order.

#### Scenario: Expected revision conflicts

- GIVEN the durable stream revision differs from the request's expected revision
- WHEN append admission runs
- THEN the adapter MUST return a typed conflict without changing journal, outbox, or revision state.

#### Scenario: Commit acknowledgement is lost

- GIVEN Aspen may have committed the batch before transport failure
- WHEN no definitive commit result reaches Kiln
- THEN the adapter MUST return `Unknown`
- AND Kiln MUST reconcile the exact stream and operation before retry classification.

### Requirement: Provider effects use exact fabric ports

r[kiln.aspen_runtime.effects] The hosted service MUST route approved provider effects only through exact Aspen fabric-port bindings with generation, effect, provider, operation, idempotency, authority, and resource linkage.

#### Scenario: A provider process effect is admitted

- GIVEN Kiln emitted an approved Mantle, Cairn, or Nix provider effect with complete bindings
- WHEN the Aspen shell routes the effect
- THEN it MUST use the selected bounded execution port
- AND it MUST return a typed observation without granting provider success.

#### Scenario: Kiln denies the provider effect

- GIVEN Kiln policy or capability admission denies an effect
- WHEN the callback result reaches the Aspen host
- THEN no process, transport, content, scheduling, or provider effect MUST execute.

### Requirement: Content and log mappings preserve exact identities

r[kiln.aspen_runtime.content] Source manifests, input roots, outputs, logs, and checkpoints MUST use bounded Aspen content references while preserving their Kiln roles and canonical identities.

#### Scenario: Output content is available

- GIVEN a provider returns output bytes and a declared output role
- WHEN the Aspen content adapter stores and verifies the bytes
- THEN the mapping MUST bind the Aspen content ref, Kiln role, producer effect, byte count, and verification receipt.

#### Scenario: Content is missing or corrupt

- GIVEN a referenced object is absent, oversized, truncated, or fails identity verification
- WHEN Kiln requests the object or evaluates completion
- THEN the adapter MUST return a typed failure
- AND Kiln MUST NOT admit successful completion.

### Requirement: Identity domains remain distinct

r[kiln.aspen_runtime.identity] Mapping code MUST preserve Kiln and Aspen identity namespaces as distinct typed values and MUST NOT infer semantic equivalence from equal BLAKE3 text.

#### Scenario: Equal digest text has different roles

- GIVEN a Kiln effect identity and an Aspen content identity contain equal digest text
- WHEN the adapter creates a mapping receipt
- THEN it MUST retain both namespaces, kinds, schemas, and roles
- AND it MUST NOT substitute one identity for the other.

### Requirement: Trigger ingress reaches durable Kiln command handling

r[kiln.aspen_runtime.ingress] Radicle and operator ingress MUST send admitted canonical triggers to the hosted Kiln service before any workflow provider dispatch.

#### Scenario: A new Radicle trigger is accepted

- GIVEN the Radicle shell acquired an exact source revision and built a valid trigger
- WHEN the Aspen client submits the trigger with complete authority and idempotency bindings
- THEN the hosted service MUST durably accept or reject the Kiln command
- AND direct Lattice dispatch MUST NOT bypass that decision.

#### Scenario: A trigger is replayed

- GIVEN the hosted service has already accepted the same trigger and idempotency key
- WHEN ingress submits it again
- THEN the service MUST return the existing run identity or a typed replay result
- AND it MUST NOT create a second provider effect.

### Requirement: Lifecycle and recovery are generation-fenced

r[kiln.aspen_runtime.lifecycle] The hosted service MUST bind activation, callbacks, effects, completions, checkpoints, recovery, drain, and shutdown to one active Aspen service generation.

#### Scenario: Service recovers after committed intent

- GIVEN journal and outbox intent committed before service interruption
- WHEN Aspen restarts the service from an admitted checkpoint and durable stream
- THEN Kiln MUST replay canonical events
- AND it MUST reconcile each unresolved effect before dispatching or completing it.

#### Scenario: A stale completion arrives

- GIVEN a completion names an inactive service generation or replaced execution identity
- WHEN completion admission runs
- THEN the service MUST reject it before state mutation.

### Requirement: Failure classes and reconciliation remain explicit

r[kiln.aspen_runtime.failure] The adapter MUST preserve domain rejection, infrastructure failure, retryable failure, timeout, cancellation, and unknown outcome as distinct results.

#### Scenario: Transport disconnects after acceptance

- GIVEN Aspen accepted a request but the client lost the acknowledgement
- WHEN the adapter classifies the transport result
- THEN it MUST report an unknown outcome with the accepted operation identity when available
- AND it MUST NOT report safe retry or success.

#### Scenario: Reconciliation finds a terminal result

- GIVEN an unknown operation later resolves to a linked terminal Aspen observation
- WHEN Kiln reconciles that exact operation
- THEN the adapter MUST return the admitted terminal observation
- AND Kiln MUST decide the resulting CI outcome.

### Requirement: Runtime selection is explicit

r[kiln.aspen_runtime.composition] Kiln MUST select the Aspen adapter, extension client, and provider mappings only at a visible application composition root.

#### Scenario: Aspen is unavailable

- GIVEN the operator selected `aspen-runtime-v1` and its endpoint or required port is unavailable
- WHEN service startup runs
- THEN startup MUST fail with a typed unavailable-provider result
- AND it MUST NOT fall back to SQLite, standalone, or direct-Lattice operation.

#### Scenario: Standalone remains selected

- GIVEN the operator selected the standalone runtime profile
- WHEN Kiln starts
- THEN it MUST run without Aspen dependencies or service discovery.

### Requirement: Conformance covers success and failure

r[kiln.aspen_runtime.conformance] Kiln MUST provide positive and negative conformance for profile admission, mapping, callback execution, durability, content, transport, effects, lifecycle, restart, reconciliation, and dependency direction.

#### Scenario: Hosted end-to-end fixture passes

- GIVEN exact compatible Kiln and Aspen cohorts with complete profiles and authority
- WHEN one accepted trigger and one rejected trigger run through the hosted service
- THEN canonical Kiln outcomes, Aspen observations, mapping receipts, and restart evidence MUST validate.

#### Scenario: A required negative fixture is missing

- GIVEN no test covers stale generation, malformed callback, atomic conflict, provider denial, timeout, cancellation, or unknown outcome
- WHEN conformance review runs
- THEN the change MUST remain incomplete.

### Requirement: Backend evidence preserves non-claims

r[kiln.aspen_runtime.nonclaims] Aspen backend evidence MUST NOT claim CI correctness, provider correctness, sandboxing, exact-once delivery, source trust, release eligibility, production availability, or global durability.

#### Scenario: A local fixture passes

- GIVEN the local hosted-service fixture and restart test pass
- WHEN operator status summarizes the result
- THEN status MUST identify the exact tested cohort and claim level
- AND it MUST retain all runtime, provider, transport, durability, and release non-claims.
