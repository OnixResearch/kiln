## Context

Kiln already plans a pure event-sourced CI core, narrow provider ports, a
durable journal and outbox, deterministic replay, and rebuildable operator
projections. The initial design names operator status and logs but does not
define their detailed behavior.

This change adapts selected concepts from Pueue revision
`b3fe223ef0a3c51d95d5c6128700b79442c54af7`, under the MIT license. The
selected concepts are task visibility, status and log commands, log following,
explicit waiting, dependency visibility, local concurrency groups, scoped
controls, crash recovery tests, and secret-redacted diagnostics.

Pueue remains a design reference only. Kiln does not import Pueue code, types,
protocols, identifiers, state, scheduler behavior, or execution authority.
Kiln does not add a Pueue provider adapter.

## Success Contract

Completion requires all of these results:

- Operator status derives from admitted Kiln events and observations.
- Status reports readiness, execution, and outcome as separate lifecycle
  facets without creating another state machine.
- Log reads and follows use bounded streams with explicit cursors, gaps,
  truncation, retention, and completion facts.
- Wait requests use named predicates over one exact selection and return a
  bounded client result without mutating CI state.
- Provider-local capacity classes use typed configuration and independent
  concurrency limits without replacing tenant fairness or provider placement.
- One pure readiness decision uses dependency facts, holds, admitted time, and
  provider-local capacity facts.
- Admission holds stop new dispatches at the selected scope without implicitly
  pausing running attempts.
- Cancellation, pause, and resume require declared provider capabilities and
  exact attempt linkage.
- Restart recovery preserves admitted log metadata and reports unresolved
  stream or control outcomes explicitly.
- Diagnostics never expose credential values, environment values, secret
  material, or unbounded provider output.
- Positive and negative fixtures cover each public decision and shell contract.
- Dependency gates prove that no Pueue crate, protocol, executable, adapter, or
  runtime requirement entered the product.

The following results are false completion:

- copying the Pueue task status enum into Kiln;
- introducing a mutable operator-only task state machine;
- using local task groups as global tenant fairness;
- reading a clock inside the pure core;
- treating a paused or held task as terminal;
- treating a client wait timeout as a task timeout;
- storing unbounded logs or hiding truncation and gaps;
- placing secret values in domain records, status, logs, or diagnostics;
- sending arbitrary interactive input to provider processes;
- editing an admitted task or retrying one attempt in place;
- importing Pueue or `pueue-lib` into any Kiln crate; or
- defining a Pueue provider profile, adapter, protocol, or conformance fixture.

## Decisions

### Decision: Show orthogonal lifecycle facets

**Choice:** Operator projections expose readiness, execution, and outcome as
separate facets.

Readiness can report held, delayed, dependency-blocked, capacity-blocked, ready,
or not-applicable. Execution can report not-dispatched, dispatching, running,
provider-paused, or not-observed. Outcome can report pending, success, domain
failure, denied, cancelled, timed out, provider error, or unknown.

The projection derives every facet from canonical events and admitted provider
observations. It does not write state or maintain a separate transition table.

**Rationale:** A single flattened status hides why work cannot start and can
confuse provider pause with admission hold or terminal completion.

### Decision: Keep operator output as a projection

**Choice:** Human output and versioned machine output use the same bounded
projection model. Filters can select projects, evaluations, runs, tasks,
attempts, providers, outcomes, and capacity classes.

Machine output is a generated interchange form. It does not define canonical
Kiln identity, event encoding, or mutation authority.

**Rationale:** Operators and automation need the same facts without making a
status database authoritative.

### Decision: Use bounded durable log streams

**Choice:** Each attempt can have declared output streams. A stream consists of
bounded ordered segments, a stable Kiln stream identity, provider observations,
BLAKE3 segment identities, and explicit retention facts.

The shell stores bytes outside the canonical event journal. The journal records
only admitted stream metadata and lifecycle facts. Every read returns a cursor,
range facts, truncation facts, gap facts, and current completion state.

A follow operation resumes from an exact cursor. Missing or expired data
returns a typed gap instead of silently starting at another position.

**Rationale:** Durable logs improve recovery and operator use. Explicit bounds
and gaps prevent log storage from becoming an unbounded hidden authority.

### Decision: Define wait as a read-only predicate

**Choice:** A wait request binds one exact selection, one named predicate, and
one configured deadline policy. The pure core evaluates the predicate over a
projection snapshot.

The shell observes projection changes and deadline facts. A client deadline
returns a client result and does not change a run, task, or attempt outcome.
Held, delayed, paused, and unknown work remains non-terminal unless the selected
predicate states otherwise.

**Rationale:** Explicit predicates remove the ambiguity of waiting for all
selected work when some work cannot currently progress.

### Decision: Adapt groups into provider-local capacity classes

**Choice:** A provider profile can declare named local capacity classes. Each
class has a named in-flight limit, admission-hold policy, ordering policy, and
unsupported-fact behavior.

A task revision can select one admitted class for one provider operation. The
pure admission decision consumes current class usage as an observation.

Capacity classes do not calculate tenant shares, select workers, or claim
provider capacity truth. Global tenant fairness runs before local capacity
admission. Providers retain worker placement.

**Rationale:** Local concurrency limits protect bounded resources without
turning a simple queue group into a global scheduler.

### Decision: Make readiness a pure decision

**Choice:** The pure readiness function consumes task state, dependency facts,
operator holds, an optional admitted time observation, local capacity facts,
and policy. It returns ready, blocked with reasons, or denied with diagnostics.

The shell supplies time and capacity observations. The core does not poll,
read clocks, sleep, or reserve provider resources.

Dependency failure policy is explicit. A policy can deny the dependent task,
mark it blocked for operator action, or admit a declared alternative path.
Silent fallback is not allowed.

**Rationale:** Pure readiness keeps delay and dependency behavior deterministic
and testable without a daemon loop.

### Decision: Separate admission holds from process controls

**Choice:** An admission hold prevents new dispatches at an exact project,
pipeline, run, task, or capacity-class scope. Release removes that hold through
a canonical command and event.

Cancellation requests a provider effect for an exact active attempt. Provider
pause and resume are optional capabilities. They never change task definitions
or reuse attempt identities.

Kiln does not add arbitrary process input as a standard control. A future
provider-specific input capability requires a separate reviewed change.

**Rationale:** Queue control and process control have different authority and
failure modes.

### Decision: Redact by construction

**Choice:** Domain records contain secret references, not secret values.
Diagnostic and `Debug` implementations use explicit safe-field projections.
They do not derive unrestricted formatting for records that can contain shell
observations.

Log metadata and status output have named text and collection bounds. Provider
output is data and never becomes an error message without bounded encoding and
redaction policy.

**Rationale:** Omission lists fail when fields change. Safe-field projections
make disclosure review explicit.

### Decision: Keep Pueue outside the product

**Choice:** Pueue remains a pinned reference in documentation and lifecycle
evidence. No crate can depend on Pueue or `pueue-lib`. No shell can invoke the
Pueue executable or implement its protocol.

No provider profile, adapter crate, test fixture, compatibility mode, feature
flag, or generated schema can name Pueue as an execution provider.

**Rationale:** The requested value is the operator behavior. Runtime adoption
would import incompatible identity, scheduling, shell, and authority semantics.

## Functional Core and Imperative Shell

Pure core functions own:

- lifecycle-facet projection from admitted state and observations;
- status filtering and stable ordering over bounded in-memory records;
- wait-predicate evaluation;
- dependency and delay readiness decisions;
- provider-local capacity admission;
- hold, release, cancellation, pause, and resume admission;
- log-cursor and gap validation over supplied metadata; and
- safe diagnostic projection from typed fields.

Thin shells own:

- journal, projection, and log-store reads;
- durable bounded log-segment writes;
- clocks, deadlines, notifications, polling, and streaming transport;
- provider usage and process observations;
- provider cancellation, pause, and resume calls;
- restart recovery and cursor materialization;
- CLI and API encoding; and
- credential and secret resolution.

Every shell must pass observed facts through a pure decision before it writes a
canonical event or dispatches a provider control effect.

## Data Flows

Status flow:

```text
canonical events + admitted provider observations
  -> pure lifecycle-facet projection
  -> bounded stable status records
  -> human or versioned machine output
```

Readiness flow:

```text
task + dependencies + holds + time observation + capacity observation + policy
  -> pure readiness decision
  -> ready, blocked reasons, or denial
  -> optional admitted dispatch effect
```

Log flow:

```text
provider output observation
  -> bounded shell capture and redaction policy
  -> durable segment + BLAKE3 segment identity
  -> admitted stream metadata
  -> bounded read or follow cursor
```

Control flow:

```text
operator command + exact scope + capability facts + current state
  -> pure control admission
  -> canonical hold event or typed provider control effect
  -> provider observation
  -> canonical completion command
```

## Configuration

Typed Nickel contracts define:

- capacity-class names, limits, ordering, and hold policy;
- log stream, segment, total-byte, retention, and read bounds;
- follow heartbeat, inactivity, and deadline policy;
- wait predicates, selection bounds, and deadline policy;
- dependency-failure and delayed-start policy;
- provider control capabilities and timeout policy; and
- diagnostic field allowlists, text bounds, and redaction classes.

Every numeric limit has a named field. Missing, zero where invalid, conflicting,
or unsupported values fail before service admission.

## Validation

Positive validation includes ready tasks, dependency completion, delayed start,
capacity availability, admission hold and release, status filtering, lifecycle
facets, bounded log reads, cursor resume, completed follow, wait completion,
cancellation, provider pause and resume, restart recovery, and safe diagnostics.

Negative validation includes dependency failure, cycles, stale time, missing
capacity facts, exhausted capacity, stale observations, unsupported controls,
stale attempts, client deadline, task timeout confusion, malformed cursors,
expired segments, gaps, truncation, oversized output, secret injection,
unbounded fields, alternate state mutation, and Pueue dependency introduction.

The dependency audit searches manifests, source imports, feature names, process
invocations, protocol names, provider registries, fixtures, and generated
schemas. Documentation and lifecycle reference records are the only allowed
Pueue mentions.

## Risks and Trade-offs

- Separate lifecycle facets increase projection complexity. They prevent hidden
  transitions and status ambiguity.
- Durable logs require retention and gap behavior. Explicit bounds make this
  cost visible.
- Capacity observations can become stale. Admission must reject or classify
  stale facts under named policy.
- Wait clients can disconnect. A disconnected read must not cancel work.
- Provider pause semantics vary. Capability profiles and outcome classes retain
  that difference.
- Reference drift can change Pueue behavior. The pinned revision limits this
  change to reviewed concepts.

## Claim Boundary

Passing this change proves bounded local relations among admitted Kiln state,
readiness facts, capacity facts, wait predicates, log metadata, control
capabilities, and operator projections for the checked implementation and
fixtures.

It does not prove provider execution, provider capacity truth, log completeness,
secret absence inside external output, process isolation, cancellation success,
pause safety, exactly-once effects, global fairness, build correctness, release
eligibility, or future Pueue behavior.
