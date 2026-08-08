## Context

Kiln starts without implementation or accepted specifications. The surrounding
stack already supplies several possible providers:

- Mantle can evaluate and realize Nix builds.
- Lattice can execute generic workflow graphs.
- Aspen/Molten can host durable distributed services.
- Valence can wrap CI facts in Evidence IR.
- Cairn can evaluate lifecycle and release gates.
- Choregraph can compile and check bounded execution machines.

None of these systems should own CI projects, pipeline revisions, evaluations,
runs, products, status, or tenant policy. GitHub and Radicle also combine forge
and collaboration behavior with Git, while Pijul has different change and
channel semantics. Jujutsu adds rewritable changes, immutable commits,
bookmarks, divergent changes, operation history, and backend-specific commit
identities. A single `SourceProvider` abstraction would erase important
differences.

Hydra shows the value of a stable project, jobset, evaluation, build, product,
and metric model. It also shows the value of fair-share scheduling and strong
operator read models. Kiln will adapt those concepts without copying Hydra's
mutable database identities, metadata files, partial graph persistence, or
control-plane coupling.

Radicle CI shows a useful decentralized model. Each node can choose which
changes it tests. Its broker maps node events to CI events, filters them, and
starts a CI-engine adapter. Kiln will support that protocol without adopting
the native runner's unisolated repository shell execution.

Other references sharpen separate boundaries. Zuul supplies exact speculative
gating. Concourse supplies version discovery and acquisition separation. The
Remote Execution API supplies content, action, operation, cache, and log
identities. SLSA, in-toto, CDEvents, and OpenTelemetry supply interchange
vocabulary. TUF supplies publication threat models. Kueue and DRF supply
multi-resource fairness tests. Temporal supplies replay-compatibility patterns.
Buildbot confirms the existing source, scheduler, request, worker, and reporter
split.

Kiln will adapt bounded semantics and invariants from these references. It will
not import their runtimes, universal abstractions, authority, or deployment
assumptions.

## Success Contract

Completion requires all of these results:

- The pure core builds and tests without Aspen, Mantle, or Lattice dependencies.
- One deterministic transition implementation decides accepted commands,
  emitted events, and requested effects.
- Terminal state cannot mutate, retries create distinct attempts, and ambiguous
  provider outcomes remain explicit.
- Provider capabilities are admitted before effects. No unsupported operation
  silently falls back.
- A standalone reference profile can execute an end-to-end fixture.
- Durable replay reconstructs the same state and emits no fresh external effect.
- Tenant admission is deterministic, supports named multi-resource strategies,
  and remains separate from provider placement.
- Exact candidate sets freeze every source revision, scope every gate result,
  and invalidate dependent results after an earlier candidate changes or fails.
- Replay compatibility checks retained histories against supported transition
  versions without emitting provider effects.
- Source discovery, acquisition, and mutation use separate capabilities.
- Optional remote-execution and standard-interchange adapters preserve Kiln
  identity and authority boundaries.
- Git, Pijul, Jujutsu, GitHub, and Radicle fit explicit source and
  collaboration ports without pretending their native identities have equal
  meaning.
- Jujutsu resolution binds one exact commit identity, preserves its change ID,
  backend kind, and observed operation identity, and rejects ambiguous revsets,
  conflicted bookmarks, and divergent changes.
- A typed hosting profile plans Radicle as canonical source governance without
  making Radicle software a core, build, test, or source-archive dependency.
- Publication evidence binds the exact candidate, RID, signed refs, configured
  governance, required replica observations, and read-only acquisition routes.
- The Radicle CI broker compatibility fixture accepts a versioned trigger,
  returns a run ID, and reports a bounded terminal result.
- Positive and negative tests cover every public transition and adapter contract.

The following results are false completion:

- importing a provider runtime into the core;
- requiring Radicle software to build or test an exact Kiln source archive;
- treating one local RID or publisher copy as completed source hosting;
- letting a writable or drifting mirror replace Radicle governance;
- performing publication, signing, synchronization, or mirror mutation during
  planning, validation, or release-candidate generation;
- using a provider run ID as a primary Kiln identity;
- treating a branch, commit, pull request, or Radicle patch as universal CI data;
- treating a Jujutsu change ID, bookmark, or revset as an immutable source revision;
- replacing separate source and effect ports with a universal resource object;
- applying a gate result to a different candidate set or moved target;
- letting a cache hit, provenance document, external event, trace, or metric
  create canonical success or release authority;
- naming Kiln as a builder when an external provider performed the build;
- importing remote-execution protocol code or a workflow runtime into the core;
- rebuilding from a moving symbolic reference after resolution;
- retrying an ambiguous external effect as if no attempt occurred;
- claiming exactly-once external execution from inbox or outbox deduplication;
- letting a read model or Radicle job COB become CI state authority;
- executing repository-provided shell text without an admitted execution policy;
- hard-coding a scheduling window, retry count, timeout, or resource weight; or
- treating passing provider tests as proof of provider correctness.

## Decisions

### Decision: Keep a specific CI domain and neutral effect boundaries

**Choice:** Kiln owns strongly typed CI concepts. It delegates all external
effects through narrow provider ports.

The initial model includes `Project`, `PipelineRevision`, `Trigger`,
`Evaluation`, `Run`, `Task`, `Attempt`, `Product`, `Metric`, and
`NotificationIntent`. It does not use a generic workflow object as the source
of CI meaning.

**Rationale:** Neutrality must apply at provider boundaries. Making the domain
vague would create a least-common-denominator workflow engine and duplicate
Lattice or Choregraph.

### Decision: Use a functional event-sourced core

**Choice:** The core exposes two deterministic relations:

```text
decide(state, command) -> decision(events, effects) | denial
evolve(state, event)   -> state | invalid-history
```

Events are durable facts. Effects are requested external actions. Replay folds
`evolve` over canonical events and never returns dispatchable effects.

**Rationale:** This split supports deterministic tests, crash recovery, audit,
and multiple shells. It also keeps provider I/O out of domain decisions.

### Decision: Keep one production transition implementation

**Choice:** CLI, API, scheduler, recovery, and adapter callbacks must call the
same production transition functions. No shell can maintain a second status or
retry state machine.

**Rationale:** Parallel state machines drift and make replay evidence weak.

### Decision: Own canonical Kiln identities

**Choice:** Kiln domain artifacts use a versioned canonical packed Preserves
profile and BLAKE3 content identities. Human aliases remain separate.
Provider-native repository, revision, actor, run, artifact, signature, and
status identifiers remain typed external references.

The codec shell verifies bytes and computes identities. The pure core receives
bounded typed identities and does not read or hash bytes.

**Rationale:** Stable CI identity cannot depend on database rows or provider
aliases. Native identities must remain available without gaining Kiln authority.

### Decision: Split provider roles

**Choice:** Kiln defines separate ports for source snapshots, collaboration,
triggers, status publication, workflow execution, build execution, event
storage, notification delivery, evidence export, and gate evaluation.
Credentials and transport handles remain shell-owned.

One adapter package may implement several ports, but each capability stays
explicit.

**Rationale:** Git, Pijul, and Jujutsu are version-control systems. GitHub is a
Git forge. Radicle is peer-to-peer Git collaboration with identity and
collaborative objects. One large provider interface would encode false
equivalence.

### Decision: Preserve Jujutsu change and commit identity

**Choice:** Jujutsu is a source provider. It is not a forge, collaboration
provider, or canonical Kiln identity authority.

A Jujutsu source observation records the repository identity, backend kind,
observed operation identity, symbolic bookmark or revset, stable change ID when
available, and one exact commit ID. Kiln binds source acquisition and evaluation
to the exact commit ID and canonical tree identity.

A change ID, bookmark, or revset can move or resolve to multiple commits. The
provider must reject ambiguous resolution, conflicted bookmarks, and divergent
changes before acquisition. The initial profile admits only the pinned Git
backend. Other backend kinds fail until a later profile admits them.

Git-backed Jujutsu commit IDs can also be Git OIDs, but their typed Jujutsu and
Git observations remain distinct provider facts.

**Rationale:** Jujutsu change IDs survive normal rewrites, while commit IDs name
exact revisions. Kiln needs both facts without promoting a moving change or
bookmark into immutable CI identity.

### Decision: Gate exact candidate sets

**Choice:** Kiln adapts Zuul's candidate-set and dependent-gate concepts without
adopting Zuul's Gerrit, Git, executor, or deployment assumptions.

A candidate set binds ordered proposed changes, a bounded dependency graph,
exact target revisions, a frozen source universe, composition observations,
pipeline revisions, and gate policy. Its canonical packed Preserves form gets a
Kiln BLAKE3 identity.

Serial gating is required. Speculative evaluation is an optional named policy.
When an earlier change fails, changes, or leaves the queue, every dependent
result becomes inapplicable. Continued changes receive new candidate identities
and new evaluations.

A successful gate requests a collaboration effect only after fresh preflight.
The preflight checks the target, candidate, approvals, provider capability, and
policy facts. The core never performs the merge.

**Rationale:** A test result is meaningful only for the exact source composition
that produced it. Provider-native merge requests do not supply a neutral
candidate identity.

### Decision: Separate discovery, acquisition, and mutation

**Choice:** Kiln adapts Concourse's versioned observation and pinning concepts,
but rejects one universal resource interface and opaque untyped configuration.

Source discovery can observe versions. Resolution binds one exact version.
Acquisition materializes that version. Mutation uses a separate provider effect
and authority check. A webhook can request discovery but cannot grant fetch,
merge, push, or publication authority.

**Rationale:** Observation and mutation have different authority. Exact version
binding improves reproducibility without erasing source-provider semantics.

### Decision: Require versioned capability admission

**Choice:** Every provider instance declares one versioned capability profile,
limits, identity namespace, supported operations, cancellation behavior, usage
facts, and outcome classes. Kiln validates the full requested operation before
emitting or dispatching an effect.

Unknown versions, absent capabilities, unsupported limits, and silent provider
substitution fail closed.

**Rationale:** Optional providers must not create hidden behavior changes.

### Decision: Keep remote execution as an optional adapter

**Choice:** Kiln can map build effects to the Bazel Remote Execution API through
an optional shell. Core types remain protocol-neutral.

The adapter preserves distinct content, input-root, command, action, operation,
execution, cache-result, log-stream, worker, and provider identities. It admits
protocol versions, digest functions, limits, cancellation, cache, execution,
and log capabilities before any upload or dispatch.

Protocol-required digests remain native external identities. Kiln canonical
manifests and products keep separate BLAKE3 identities. A cache hit is an
observation, not proof of execution or product correctness.

**Rationale:** These distinctions strengthen every build adapter. Mantle and
standalone providers still own build execution, cache behavior, and workers.

### Decision: Keep global admission separate from local placement

**Choice:** Kiln owns tenant shares, project admission policy, and global ready
ordering. A provider owns worker selection, queue implementation, and local
capacity placement.

The baseline fairness score is derived from recent normalized resource use
divided by configured share. The profile names weighted-scalar or dominant-share
dimension aggregation. The accounting window, capacity basis, decay, resource
weights, rounding, stale-usage policy, and tie-break rules are named
configuration.

Holding other facts equal, additional positive usage cannot improve a tenant's
priority. Missing or incomparable resource dimensions fail under explicit
policy.

**Rationale:** Kiln adapts Kueue and DRF as fairness references, not placement
engines. Build and workflow providers still own worker selection and local
capacity.

### Decision: Model ambiguous outcomes explicitly

**Choice:** Provider results distinguish success, domain failure, denied,
cancelled, timed out, provider error, and unknown outcome. An unknown outcome
keeps the attempt non-terminal until reconciliation or operator policy resolves
it. A retry creates a new attempt identity.

**Rationale:** A transport failure after effect submission does not prove that
the provider did nothing.

### Decision: Use a durable journal and transactional outbox

**Choice:** The shell appends accepted events with optimistic concurrency and
records requested effects in the same local commit. Workers claim effects with
leases and idempotency keys. Completion enters through a new command and event.

Read models, metrics, logs, and status pages are rebuildable projections.
Deduplication proves only bounded duplicate suppression under the recorded
store and retention policy.

**Rationale:** Lifecycle events must survive restarts. Atomic event and outbox
recording prevents accepted state from losing its requested effect.

### Decision: Gate transition changes with retained-history replay

**Choice:** Kiln adapts Temporal's command-history audit pattern as a release
check, but retains its own `decide` and `evolve` model.

Every supported event and transition version must replay through the current
production evolution path or an explicit deterministic migration. Retained
history divergence fails service and release admission. Replay still emits no
provider effect.

**Rationale:** Deterministic code today does not prove compatibility with stored
history after code changes.

### Decision: Provide a standalone reference profile

**Choice:** Kiln includes a local event store, deterministic fixture providers,
and a bounded local service shell. This profile is sufficient for conformance
and end-to-end tests without Aspen, Mantle, or Lattice.

It is a reference implementation, not an automatic production profile.

**Rationale:** A neutral core is credible only if it runs without the providers
that it claims are optional.

### Decision: Export standards without transferring authority

**Choice:** Kiln provides versioned projections for SLSA and in-toto provenance,
CDEvents over CloudEvents, and OpenTelemetry CI/CD telemetry.

Provenance binds exact source, candidate, pipeline, dependencies, producing
attempt, provider trust facts, products, metrics, and byproducts. Builder,
signer, provider, worker, and Kiln identities remain separate. Kiln is not the
builder when Mantle or another provider performs the build.

CDEvents imports remain external observations until a supported pure mapping
produces an admitted Kiln command. OpenTelemetry spans, metrics, and logs remain
rebuildable projections. Neither surface becomes journal, lifecycle, gate, or
release authority.

**Rationale:** Standards improve interoperability. Valence retains shared
evidence semantics, while Kiln retains CI state and identity authority.

### Decision: Host source on Radicle without binding the runtime

**Choice:** Radicle is the planned canonical source and collaboration host for
Kiln. Canonical repository authority comes from the admitted Radicle project
identity, configured delegates and threshold, required signed-reference feature
profile, and exact governed source revision.

A typed Nickel hosting profile records planned and observed state. Before
publication, its RID remains explicitly unassigned. A local candidate binds the
exact source revision, canonical archive BLAKE3 identity, license, lifecycle
state, release evidence, rollback revision, and hosting profile.

Publication requires a separate explicit operator action. The authorized shell
may then initialize the Radicle project and publish only the unchanged
candidate. It records the assigned RID, signed refs, configured governance,
required native replica observations, and fresh native and read-only acquisition
results.

GitHub, HTTPS, and other mirrors are declared projections. Each mirror profile
states direction, mutability, synchronization, and authority. A mirror cannot
replace Radicle governance through availability, drift, or a successful write.

Exact source archives must build and test without a Radicle node, `rad`, a
Radicle identity, or network access. Repository recovery cannot depend on a
running Kiln service or a passing Kiln-hosted CI result.

The Radicle CI adapter remains a separate optional product integration. Source
hosting does not enable that adapter and does not add Radicle types to Kiln core.

**Rationale:** Repository governance and runtime provider choice are different
boundaries. Radicle can host Kiln while Kiln remains usable, testable, and
portable without Radicle runtime components.

Automatic authority migration is rejected. Seed loss, delegate loss, mirror
drift, and governance change require explicit reviewed recovery actions.

Kiln adapts TUF's rollback, freeze, mix-and-match, threshold, delegation, and
malicious-mirror threat questions. It does not implement TUF metadata or replace
Radicle governance. One receipt binds the full publication set. Named freshness
and monotonic sequence policy rejects stale, mixed, or rolled-back observations.

### Decision: Support Radicle CI in two modes

**Choice:** The first mode is a compatibility executable for the Radicle CI
broker versioned line-JSON protocol. It maps a broker trigger to a Kiln command,
returns the Kiln run ID, waits or polls through bounded policy, and returns the
protocol result.

A separate status adapter can project Kiln state into Radicle job COBs. A later
direct mode may consume Radicle node events without the broker. Direct mode
must use a separate capability profile and must not silently replace broker
mode.

**Rationale:** Broker compatibility gives early integration and preserves
node-local filtering. Direct mode can add richer asynchronous behavior later.

### Decision: Do not adopt Radicle native execution semantics

**Choice:** Kiln may read the Radicle broker protocol, but it will not execute
`.radicle/native.yaml` shell text as an implicit trusted action. A compatibility
import must lower repository configuration into an admitted pipeline revision
and an explicit execution-provider request.

**Rationale:** The native runner states that it has no isolation and is suitable
only for trusted repositories. Kiln must keep trust and execution policy
explicit.

### Decision: Use Nickel for human-authored configuration

**Choice:** Projects, providers, capabilities, fairness, retry, retention,
repository hosting, and operator policy use typed Nickel contracts. Deterministic generated projections
may use a provider's required format.

Radicle broker messages and GitHub webhook signatures retain their native
protocol formats and algorithms.

**Rationale:** Nickel catches invalid configuration before the service applies
it. Protocol compatibility does not justify a second configuration authority.

## Functional Core and Imperative Shell

Pure core functions own:

- identifier, entity, and lifecycle admission;
- command decisions and event evolution;
- provider capability matching;
- retry, cancellation, and reconciliation decisions;
- deterministic ready ordering and fairness scores;
- candidate composition, gate invalidation, and merge preflight;
- source-resolution and resource-observation validation;
- remote-execution and interchange mapping data;
- status and read-model projection data; and
- replay and retained-history compatibility without effect output.

Thin shells own:

- files, databases, transactions, locks, and migrations;
- clocks, timers, randomness, and process lifecycle;
- network listeners, webhooks, and provider API calls;
- repository discovery, fetch, snapshot materialization, and mutation;
- remote-execution transport, CAS transfer, operation polling, and log reads;
- interchange encoding, transport, signing, and telemetry delivery;
- credentials, signatures, and native identity verification;
- canonical byte encoding and BLAKE3 calculation;
- effect leasing and dispatch; and
- logs, metrics, API rendering, and notification transport.

Every shell must pass observed facts into a pure decision function before a
state mutation or external dispatch.

## Data and Identity Flow

```text
provider event or operator command
  -> shell authentication and native parsing
  -> canonical bounded Kiln command
  -> decide(current state, command)
  -> durable Kiln events + typed effect requests
  -> journal and outbox commit
  -> admitted provider dispatch
  -> provider observation or callback
  -> canonical completion command
  -> next Kiln events
  -> rebuildable status and evidence projections
```

For source material:

```text
symbolic provider reference
  -> provider resolution
  -> exact native revision + materialized tree bytes
  -> canonical tree manifest + BLAKE3 tree identity
  -> source-resolution observation
  -> evaluation identity
```

The tree identity proves content identity only. It does not prove equal history,
review state, signatures, trust, or merge semantics.

For change gating:

```text
exact targets + ordered changes + dependency graph + pipeline revisions
  -> frozen source universe + composition observations
  -> canonical candidate-set identity
  -> serial or admitted speculative evaluations
  -> candidate-scoped gate results
  -> fresh merge preflight
  -> collaboration-provider merge effect request
```

A moved target or invalidated prefix creates a new candidate. Earlier results do
not transfer to the new identity.

For repository hosting:

```text
validated local source revision + license + release evidence
  -> immutable publication candidate + archive BLAKE3
  -> explicit operator publication authority
  -> Radicle RID + governed signed refs
  -> configured replica observations
  -> fresh native and read-only acquisition equality
  -> non-authoritative mirror projections
```

Planning stops before the operator-authority arrow. Validation and candidate
generation perform no remote mutation.

## Provider Composition

A deployment can select independent providers:

| Role | Example |
|---|---|
| Source | Git, Pijul, or Jujutsu |
| Collaboration | GitHub or Radicle |
| Trigger | GitHub webhook, Radicle broker, local hook, or polling |
| Workflow | Lattice or standalone runner |
| Build | Mantle, Remote Execution API adapter, or standalone executor |
| Runtime | Aspen/Molten or local service shell |
| Evidence | Valence, SLSA/in-toto projection, or local canonical records |
| Event interchange | CDEvents adapter or local hooks |
| Telemetry | OpenTelemetry adapter or local projection |
| Gate | Cairn or a local policy gate |

Provider packages may live in the Kiln workspace, but provider dependencies
must not appear in core manifests or public core types.

## Compatibility and Migration

Kiln has no prior public schema. Every initial schema and port still uses an
explicit version.

- Unknown domain, artifact, event, command, provider, and protocol versions fail.
- Human aliases never replace stable identities.
- Moving source references are recorded but never re-resolved for an existing
  evaluation.
- Provider-native status remains a projection or observation.
- Candidate results remain bound to one exact candidate-set identity.
- Remote-execution identities remain external references.
- SLSA, in-toto, CDEvents, CloudEvents, and OpenTelemetry remain projections or
  observations and do not define canonical Kiln identity or state.
- The Radicle broker protocol remains a compatibility shell, not Kiln's
  internal event format.
- JSON and HTML remain projections and do not define canonical identity.

## Validation

Positive validation includes valid project admission, immutable pipeline
revisions, accepted transitions, retries, cancellation, replay, capability
matching, Git, Pijul, and Jujutsu source resolution, candidate composition,
serial and speculative
gating, invalidation, merge preflight, standalone execution, weighted and
dominant-share fairness, retained-history replay, remote-execution mapping,
interchange projections, planned and published hosting profiles, exact
acquisition, Radicle trigger mapping, and status projection.

Negative validation includes malformed identities, duplicate events, terminal
mutation, stale revisions, moving-reference drift, ambiguous Jujutsu revsets,
conflicted bookmarks, divergent changes, unsupported capabilities,
provider substitution, invalid signatures, replayed triggers, stale usage,
zero or invalid shares, ambiguous outcomes, outbox interruption, event-history
corruption, Radicle protocol noise, missing commits, unsafe shell imports,
missing or stale RID data, governance mismatch, signed-ref weakness, mixed
publication bundles, rollback, freeze, replica shortfall, mirror drift, writable
mirrors, circular bootstrap, candidate cycles, stale gate results, source drift,
remote identity confusion, cache overclaim, builder confusion, lossy interchange,
replay divergence, and overclaiming evidence.

The maintained rail will include formatting, workspace tests, strict Clippy,
`no_std` core checks, Nickel contract tests, adapter dependency audits, Cairn
validation, Tracey coverage, protocol fixtures, and focused Nix checks.

## Risks / Trade-offs

- **Abstraction cost:** Several narrow ports require more adapter code. They
  preserve provider differences and stop authority leakage.
- **Event migration:** Durable events require strict versioning and migration
  tools. Rebuildable projections reduce migration scope.
- **Cross-provider fairness:** Usage units can be incomparable. Capability
  profiles must expose exact normalization and deny unsupported policy.
- **Speculative waste:** Failed candidate prefixes can waste execution. Named
  windows and fairness interaction bound this cost.
- **Standard drift:** CDEvents and OpenTelemetry conventions can change. Pinned
  profiles and explicit evolution policy prevent silent semantic changes.
- **Remote protocol breadth:** Remote execution has many capabilities. Optional
  profiles must reject unsupported behavior before effects.
- **Radicle hosting availability:** Observed replicas and acquisitions do not
  prove future availability. Recovery policy must keep this limit explicit.
- **Radicle governance loss:** Delegate or seed loss can block normal updates.
  Recovery must not create automatic replacement authority.
- **Mirror confusion:** Familiar GitHub or HTTPS URLs can appear canonical.
  Hosting profiles and documentation must label them as projections.
- **Radicle protocol limits:** Broker version 1 has a small result model and a
  process-bound lifecycle. The compatibility adapter must record those limits.
- **Standalone profile:** A reference shell can be mistaken for a secure hosted
  service. Documentation and admission policy must prevent that promotion.
- **Canonical format:** Packed Preserves adds a codec dependency. Keeping it out
  of the core transition logic limits the coupling.

## Claim Boundary

Passing this change proves bounded local relations among admitted Kiln state,
commands, events, provider capabilities, effect requests, replay, and canonical
artifacts for the checked implementation and fixtures.

It does not prove source-history truth, provider correctness, signature trust,
credential freshness, sandboxing, build correctness, workflow correctness,
external effect success, exactly-once execution, global fairness, candidate
correctness outside configured checks, remote cache or worker correctness,
provenance completeness, SLSA level, telemetry completeness, delegate honesty,
future Radicle or mirror availability, universal write exclusion, artifact
retention, release eligibility, or deployment safety.
