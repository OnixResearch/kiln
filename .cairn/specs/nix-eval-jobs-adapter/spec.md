# Nix Eval Jobs Adapter Specification

## Purpose

Defines the `nix-eval-jobs-adapter` capability.

## Requirements

### Requirement: Optional adapter and dependency boundary

r[kiln.nix_eval_jobs.dependency_boundary] Kiln MUST implement `nix-eval-jobs` support as an optional workflow-discovery adapter. Kiln core crates MUST NOT import Nix, Hydra, or `nix-eval-jobs` code, types, protocols, or private libraries.

#### Scenario: Adapter is not configured

- **GIVEN** a Kiln deployment without an admitted `nix-eval-jobs` profile
- **WHEN** core and standalone provider checks run
- **THEN** they MUST build and pass without a Nix installation or evaluator executable

#### Scenario: Core imports Nix implementation code

- **GIVEN** a core manifest or source file imports Nix, Hydra, or `nix-eval-jobs` implementation code
- **WHEN** the dependency boundary gate runs
- **THEN** the gate MUST fail and identify the forbidden edge

### Requirement: Exact executable and evaluator cohort

r[kiln.nix_eval_jobs.cohort_admission] The adapter MUST admit one exact absolute executable path, executable BLAKE3, upstream revision, adapter version, Nix version, platform, and argument policy before launch. It MUST NOT resolve the executable through ambient `PATH` or fall back to another evaluator.

#### Scenario: Exact cohort is available

- **GIVEN** the executable bytes, versions, platform, and policy match the selected profile
- **WHEN** adapter preflight runs
- **THEN** the adapter MAY launch the exact executable
- **AND** its observation MUST bind every admitted cohort fact

#### Scenario: Executable bytes drift

- **GIVEN** the configured path resolves to bytes with a different BLAKE3 or Nix version
- **WHEN** adapter preflight runs
- **THEN** the adapter MUST reject before jobset evaluation
- **AND** it MUST NOT select an ambient or prior executable

### Requirement: Immutable source and selector admission

r[kiln.nix_eval_jobs.input_admission] Every evaluation request MUST bind an immutable source identity, expression or flake mode, explicit root selector, target system, evaluation options, and policy identity. Floating references, implicit registries, unrestricted callbacks, and undeclared import authority MUST fail before launch.

#### Scenario: Locked flake source is selected

- **GIVEN** an admitted immutable flake source, supported selector, system, and option set
- **WHEN** request admission runs
- **THEN** it MUST produce one bounded adapter request with a stable identity preimage

#### Scenario: Source reference can advance

- **GIVEN** a branch, tag, registry alias, or other source reference without an admitted immutable resolution
- **WHEN** request admission runs
- **THEN** the request MUST be rejected before process launch
- **AND** no job or task identity may be created

### Requirement: Bounded incremental stream admission

r[kiln.nix_eval_jobs.stream_admission] The adapter MUST read complete newline-delimited JSON records under named line, total-byte, record-count, metadata, diagnostic, dependency, and collection bounds. Malformed, truncated, duplicate, unsupported, or oversized records MUST fail closed.

#### Scenario: Several valid records arrive

- **GIVEN** the pinned executable emits supported complete records within every named bound
- **WHEN** the adapter reads the stream
- **THEN** each line MUST enter schema admission independently
- **AND** the shell MUST NOT require one unbounded complete-output buffer

#### Scenario: One line is truncated

- **GIVEN** the process exits or closes stdout within a JSON record
- **WHEN** stream admission observes EOF
- **THEN** it MUST classify the stream as incomplete
- **AND** it MUST NOT accept the partial line or report complete evaluation

### Requirement: Terminal completeness uses process facts

r[kiln.nix_eval_jobs.terminal_completeness] Parsed job records MUST remain provisional until the adapter classifies EOF, process exit, timeout, signal, stderr, teardown, and GC-root facts. A crash or ambiguous process outcome MUST NOT become a complete jobset observation.

#### Scenario: Process exits successfully after complete records

- **GIVEN** all records pass admission and the owned process exits with the configured success status
- **WHEN** terminal classification runs
- **THEN** the adapter MAY mark the jobset observation complete
- **AND** it MUST bind the process and stream observations

#### Scenario: Process dies after useful records

- **GIVEN** several records pass admission before the process crashes or times out
- **WHEN** terminal classification runs
- **THEN** those records MUST remain partial diagnostic observations
- **AND** they MUST NOT imply complete evaluation or automatic safe retry

### Requirement: Pure task-proposal mapping

r[kiln.nix_eval_jobs.task_mapping] Successful derivation records MUST enter a pure bounded mapping decision before they can become Kiln task proposals. Task identities MUST bind exact source, selector, system, attribute, derivation, output, schema, and mapping-policy facts.

#### Scenario: Valid derivation job maps to a task

- **GIVEN** one admitted successful job record with supported derivation and output facts
- **WHEN** pure task mapping runs
- **THEN** it MUST return one bounded task proposal with separate Kiln and provider-native identities
- **AND** canonical Kiln command admission MUST remain required

#### Scenario: Job identities conflict

- **GIVEN** duplicate attributes, conflicting derivations, missing required outputs, a dependency cycle, or an exceeded task bound
- **WHEN** task mapping runs
- **THEN** it MUST return stable blockers without events or provider effects

### Requirement: Job and process failures remain distinct

r[kiln.nix_eval_jobs.failure_scope] A per-job evaluation error MUST remain distinct from fatal setup failure, protocol failure, process loss, cancellation, timeout, and unknown outcome. A valid job error MUST NOT suppress independent valid job mapping.

#### Scenario: One attribute fails evaluation

- **GIVEN** the complete stream contains one supported job-error record and one successful independent derivation record
- **WHEN** mapping and terminal classification run
- **THEN** Kiln MUST preserve the failed job observation
- **AND** it MAY map the successful independent job under the selected complete-stream policy

#### Scenario: Evaluator startup fails

- **GIVEN** the evaluator fails before it can produce valid job records
- **WHEN** adapter outcome classification runs
- **THEN** it MUST return one fatal provider observation
- **AND** it MUST NOT invent a failure for each unknown job

### Requirement: Cache observations remain advisory

r[kiln.nix_eval_jobs.cache_advisory] Upstream cache and build-status fields MUST remain bounded advisory observations. They MUST NOT complete tasks, skip provider admission, authorize substitution, or prove output presence.

#### Scenario: Upstream reports a cached job

- **GIVEN** an admitted job record reports cached or substitutable outputs
- **WHEN** Kiln maps cache fields
- **THEN** it MAY emit an advisory metric or scheduling observation with source and freshness facts
- **AND** task completion MUST still require admitted build-provider observations

#### Scenario: Deprecated cache field conflicts

- **GIVEN** deprecated and current cache fields disagree
- **WHEN** cache mapping runs
- **THEN** the adapter MUST report a stable compatibility conflict
- **AND** it MUST NOT select the favorable value silently

### Requirement: GC-root ownership and handoff

r[kiln.nix_eval_jobs.gc_root_handoff] When derivation retention is enabled, the adapter MUST use a private run-scoped GC-root directory and MUST record creation, ownership, handoff, retention, and cleanup outcomes. It MUST NOT remove a required root before checked downstream handoff.

#### Scenario: Build provider accepts a task

- **GIVEN** an admitted task depends on a retained Nix derivation and the downstream provider confirms an accepted durable handoff
- **WHEN** GC-root retention planning runs
- **THEN** the adapter MAY release the corresponding evaluator root under policy
- **AND** it MUST record the handoff and cleanup result

#### Scenario: Cleanup fails

- **GIVEN** evaluation or handoff completes but one owned GC root cannot be removed safely
- **WHEN** terminal reporting runs
- **THEN** the report MUST preserve an explicit cleanup failure or retained state
- **AND** it MUST NOT claim clean resource completion

### Requirement: Onix Nix remains unsupported without evidence

r[kiln.nix_eval_jobs.onix_boundary] The initial adapter profile MUST NOT claim compatibility with Onix Nix, `builtins.wasm`, fork-specific evaluator behavior, alternate store semantics, or unsupported Nix cohorts.

#### Scenario: Jobset uses an Onix-only builtin

- **GIVEN** a jobset requires `builtins.wasm` but the selected profile admits only the pinned stock Nix cohort
- **WHEN** request or fixture admission runs
- **THEN** the adapter MUST reject or report the cohort as unsupported
- **AND** it MUST NOT classify the result as general Nix evaluation failure

#### Scenario: Future Onix cohort is proposed

- **GIVEN** a candidate profile for an Onix Nix evaluator cohort
- **WHEN** compatibility admission runs
- **THEN** it MUST require an exact binary identity and independent positive and negative cohort fixtures

### Requirement: Pure adapter decisions and narrow shell

r[kiln.nix_eval_jobs.core_shell_boundary] Profile admission, schema admission, task mapping, identity preimages, dependency checks, partial-policy decisions, and outcome classification MUST be pure. Executable access, processes, pipes, clocks, cancellation, hashing, GC-root effects, and provider observations MUST remain in shells.

#### Scenario: Equal records are replayed

- **GIVEN** byte-equivalent admitted profile, job, policy, and Kiln identity facts
- **WHEN** pure task mapping runs twice
- **THEN** both decisions MUST be equal
- **AND** the core MUST perform no filesystem, process, network, environment, clock, credential, or output effect

#### Scenario: Shell tries direct task creation

- **GIVEN** the process shell parsed a valid upstream job record without a pure mapping decision
- **WHEN** it attempts to create a canonical task or dispatch a build effect
- **THEN** the architecture gate MUST reject that path

### Requirement: Adapter conformance and non-claims

r[kiln.nix_eval_jobs.conformance] The adapter MUST pass shared provider conformance plus positive and negative cohort, input, stream, mapping, process, cache, GC-root, dependency, and runtime-independence fixtures.

#### Scenario: Supported matrix passes

- **GIVEN** the pinned cohort and complete valid, job-error, cache, dependency, and GC-root fixtures
- **WHEN** focused adapter conformance runs
- **THEN** every fixture MUST produce its expected typed observations, proposals, and terminal class

#### Scenario: Negative fixture fails elsewhere

- **GIVEN** a malformed-stream fixture that fails because an unrelated source or build provider is unavailable
- **WHEN** conformance classifies the result
- **THEN** the fixture MUST NOT count as stream-boundary rejection evidence

r[kiln.nix_eval_jobs.nonclaims] Passing adapter checks MUST NOT claim Nix evaluation correctness, source truth, complete dependency semantics, cache truth, build execution, output correctness, Onix Nix compatibility, reproducibility, deployment safety, or release eligibility.

#### Scenario: Pinned integration fixture passes

- **GIVEN** one local jobset passes under the exact admitted executable and Nix cohort
- **WHEN** Kiln emits compatibility evidence
- **THEN** evidence MUST remain limited to the checked inputs, process, records, mapping, policy, and local observations
