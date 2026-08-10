# Task Operations Specification

## Requirements

### Requirement: Reference-only adaptation boundary
r[kiln.task_operations.reference_boundary] Kiln MUST implement task-operations behavior without a Pueue or `pueue-lib` dependency, Pueue protocol type, Pueue executable invocation, Pueue provider profile, or Pueue adapter.

#### Scenario: Product code imports the reference
r[kiln.task_operations.reference_boundary.scenario.import]
- GIVEN a Kiln manifest, source file, generated schema, fixture, or provider registry that introduces Pueue product integration
- WHEN the dependency and compatibility boundary gate runs
- THEN the gate MUST fail and MUST identify the forbidden integration surface.

#### Scenario: Documentation records the source reference
r[kiln.task_operations.reference_boundary.scenario.documentation]
- GIVEN documentation that names the pinned Pueue revision, selected concepts, rejected concepts, license, and non-claims
- WHEN the reference boundary gate runs
- THEN the documentation MUST remain allowed without creating a product dependency.

### Requirement: Rebuildable operator status
r[kiln.task_operations.status] Kiln MUST derive bounded operator status from canonical events and admitted observations, and status output MUST NOT become mutation authority.

#### Scenario: Status projection is rebuilt
r[kiln.task_operations.status.scenario.rebuild]
- GIVEN an intact journal and admitted provider observations with no derived status store
- WHEN projection rebuild runs
- THEN it MUST produce the same ordered status records without dispatching effects.

#### Scenario: Status storage contains an unsupported mutation
r[kiln.task_operations.status.scenario.mutation]
- GIVEN a derived status record that conflicts with canonical events
- WHEN Kiln reads operator status
- THEN Kiln MUST reject or rebuild the derived record and MUST NOT mutate canonical state from it.

### Requirement: Orthogonal lifecycle visibility
r[kiln.task_operations.lifecycle_facets] Operator status MUST expose readiness, execution, and outcome as separate typed facets derived through the production transition and projection paths.

#### Scenario: Task waits for capacity
r[kiln.task_operations.lifecycle_facets.scenario.capacity]
- GIVEN a non-terminal task with satisfied dependencies and unavailable admitted local capacity
- WHEN Kiln builds its status projection
- THEN readiness MUST report a capacity block while execution remains not dispatched and outcome remains pending.

#### Scenario: Provider pause is reported as completion
r[kiln.task_operations.lifecycle_facets.scenario.pause]
- GIVEN an active attempt with an admitted provider-paused observation
- WHEN Kiln builds its status projection
- THEN execution MUST report provider-paused and outcome MUST remain non-terminal.

### Requirement: Bounded durable log streams
r[kiln.task_operations.logs] Kiln MUST model attempt logs as bounded durable streams with stable Kiln stream identities, ordered segment metadata, BLAKE3 segment identities, retention facts, and explicit truncation and gap facts.

#### Scenario: Log stream survives shell restart
r[kiln.task_operations.logs.scenario.restart]
- GIVEN committed log segments and admitted stream metadata for an active attempt
- WHEN the shell restarts and recovers the stream
- THEN a bounded read MUST return the same retained bytes and cursor facts without changing CI state.

#### Scenario: Requested log bytes expired
r[kiln.task_operations.logs.scenario.expired]
- GIVEN a cursor whose referenced segment is outside the admitted retention set
- WHEN an operator requests the next log range
- THEN the read MUST return a typed gap or expiration result and MUST NOT silently move the cursor.

### Requirement: Explicit log cursors and follow completion
r[kiln.task_operations.follow] Every log read or follow operation MUST bind one exact stream, one admitted cursor or start rule, named bounds, and explicit current completion state.

#### Scenario: Follow reaches stream completion
r[kiln.task_operations.follow.scenario.complete]
- GIVEN a valid cursor for a stream with newly retained segments and an admitted terminal stream observation
- WHEN follow reads through the terminal segment
- THEN it MUST return the bounded segments, the final cursor, and terminal stream state.

#### Scenario: Cursor names another stream
r[kiln.task_operations.follow.scenario.mismatch]
- GIVEN a cursor whose stream identity differs from the selected attempt stream
- WHEN follow admission runs
- THEN it MUST reject the request without reading bytes or changing either stream.

### Requirement: Read-only wait predicates
r[kiln.task_operations.wait] A wait request MUST bind one exact bounded selection, one named predicate, and one deadline policy, and wait completion MUST NOT mutate CI state.

#### Scenario: Selected attempts become terminal
r[kiln.task_operations.wait.scenario.terminal]
- GIVEN a wait predicate that requires all selected attempts to become terminal
- WHEN admitted projection updates satisfy that predicate
- THEN the wait shell MUST return the matching bounded status snapshot.

#### Scenario: Client deadline expires
r[kiln.task_operations.wait.scenario.deadline]
- GIVEN a non-terminal selected attempt and an admitted client deadline observation
- WHEN the wait shell reaches that deadline
- THEN it MUST return a client timeout result and MUST NOT mark the attempt timed out or cancelled.

### Requirement: Provider-local capacity classes
r[kiln.task_operations.capacity] A provider profile MUST define typed provider-local capacity classes with named in-flight limits, ordering, hold policy, freshness policy, and unsupported-fact behavior.

#### Scenario: Local class has available capacity
r[kiln.task_operations.capacity.scenario.available]
- GIVEN an admitted task bound to one provider-local capacity class with fresh usage below its configured limit
- WHEN local capacity admission runs
- THEN the task MUST remain eligible under that local capacity decision.

#### Scenario: Capacity class replaces tenant fairness
r[kiln.task_operations.capacity.scenario.fairness]
- GIVEN a policy that uses a local capacity class as a tenant share or global fairness score
- WHEN configuration admission runs
- THEN it MUST reject the policy before task admission.

### Requirement: Pure task readiness
r[kiln.task_operations.readiness] Kiln MUST decide task readiness through one pure function over task state, dependency facts, operator holds, admitted time, provider-local capacity facts, and typed policy.

#### Scenario: All readiness conditions pass
r[kiln.task_operations.readiness.scenario.ready]
- GIVEN a non-terminal task with satisfied dependencies, no matching hold, admitted delay completion, and available local capacity
- WHEN the readiness function runs
- THEN it MUST return ready with no shell effect.

#### Scenario: Shell state changes readiness directly
r[kiln.task_operations.readiness.scenario.shell_mutation]
- GIVEN a shell-local queue record that is absent from canonical events and admitted observations
- WHEN readiness is decided
- THEN the record MUST have no authority to make the task ready or blocked.

### Requirement: Explicit dependency and delay facts
r[kiln.task_operations.blocking_facts] Dependency blocks, dependency failures, delayed starts, and capacity blocks MUST remain distinct typed readiness facts with exact source identities and policy versions.

#### Scenario: Dependency completes after the delay
r[kiln.task_operations.blocking_facts.scenario.release]
- GIVEN a delayed task whose exact dependencies are now successful and whose admitted time observation passes the delay boundary
- WHEN readiness is decided
- THEN both blocks MUST clear before the task can become ready.

#### Scenario: Core reads wall-clock time
r[kiln.task_operations.blocking_facts.scenario.clock]
- GIVEN a delayed task without an admitted time observation
- WHEN readiness is decided
- THEN the core MUST return a missing-time denial and MUST NOT read a clock or guess delay completion.

### Requirement: Admission holds and scoped controls
r[kiln.task_operations.controls] Kiln MUST separate admission holds from provider process controls and MUST admit cancellation, pause, or resume only for exact active attempts and declared provider capabilities.

#### Scenario: Operator holds a capacity class
r[kiln.task_operations.controls.scenario.hold]
- GIVEN an admitted hold for one provider-local capacity class
- WHEN readiness runs for a matching queued task and an unrelated running attempt
- THEN the queued task MUST remain held and the running attempt MUST remain unchanged.

#### Scenario: Provider lacks pause capability
r[kiln.task_operations.controls.scenario.unsupported]
- GIVEN an active attempt on a provider that does not declare pause support
- WHEN an operator requests pause
- THEN control admission MUST deny the request without dispatching an effect or changing execution state.

### Requirement: Secret-redacted diagnostics
r[kiln.task_operations.redaction] Domain, status, log-metadata, error, and `Debug` output MUST use explicit safe-field projections with named bounds and MUST NOT expose credential values, environment values, or secret material.

#### Scenario: Safe diagnostic is rendered
r[kiln.task_operations.redaction.scenario.safe]
- GIVEN a failed provider observation with safe typed identifiers and a bounded public diagnostic
- WHEN Kiln renders operator output
- THEN it MUST include the admitted safe fields and exact redaction markers.

#### Scenario: Provider error contains a secret value
r[kiln.task_operations.redaction.scenario.secret]
- GIVEN a provider error or environment observation that contains admitted secret test material
- WHEN diagnostic rendering runs
- THEN the output MUST omit the secret value and MUST return the configured redaction marker or bounded safe diagnostic.

### Requirement: Positive and negative task-operations fixtures
r[kiln.task_operations.fixtures] The maintained suite MUST include positive and negative fixtures for status, lifecycle facets, logs, follow, wait, capacity, readiness, delays, dependencies, controls, restart recovery, redaction, bounds, and reference independence.

#### Scenario: Negative fixtures are absent
r[kiln.task_operations.fixtures.scenario.missing_negative]
- GIVEN a release candidate without current malformed-cursor, stale-capacity, client-deadline, secret-injection, unsupported-control, and forbidden-dependency fixtures
- WHEN task-operations readiness runs
- THEN the candidate MUST fail before release admission.

### Requirement: Task-operations non-claims
r[kiln.task_operations.nonclaims] Passing task-operations checks MUST NOT claim provider execution, provider capacity truth, log completeness, secret absence in external output, process isolation, control success, exactly-once effects, global fairness, build correctness, or release eligibility.

#### Scenario: Recovered log stream is complete within retention
r[kiln.task_operations.nonclaims.scenario.logs]
- GIVEN a recovered stream whose retained segments and metadata are internally consistent
- WHEN Kiln reports recovery evidence
- THEN the report MUST remain limited to the retained bytes, recorded gaps, selected policy, and observed stream state.
