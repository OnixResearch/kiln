# Nix Build Specification Delta

## ADDED Requirements

### Requirement: Exact Nix build cohort

r[kiln.nix_build.cohort] The Nix build adapter MUST admit one pinned nixpkgs revision, Nix version, `nix` and `nix-store` executable paths and BLAKE3 identities, request/observation/receipt/native schemas, command, platform, Bounded Exec revision, limits, and non-claims before any process starts.

#### Scenario: Pinned cohort is admitted

r[kiln.nix_build.cohort.scenario.pinned]
- GIVEN the repo-pinned Nix executable and store executable identities
- WHEN profile admission runs
- THEN the profile MUST be admitted
- AND the provider MUST declare the build role, dispatch/observe/cancel/read-logs/reconcile operations, and all terminal outcome classes

#### Scenario: Cohort or limit drift fails closed

r[kiln.nix_build.cohort.scenario.drift]
- GIVEN a changed executable identity, `nix-store` identity, derivation identity, version, capability set, output order, or inconsistent limit
- WHEN profile admission or adapter construction runs
- THEN it MUST fail before any build process starts

### Requirement: Exact derivation and output request

r[kiln.nix_build.request] The adapter MUST require the build plan's provider profile, plan identity, policy identity, platform, and sorted product names to equal the exact admitted runtime binding, and MUST bind every expected product name and store path before execution.

#### Scenario: Exact plan is lowered

r[kiln.nix_build.request.scenario.exact]
- GIVEN a plan that matches the profile and one safe derivation child
- WHEN request lowering runs
- THEN the request MUST contain the exact derivation path and required product names
- AND the profile MUST retain the exact expected store path for every product

#### Scenario: Plan or output mismatch fails before execution

r[kiln.nix_build.request.scenario.mismatch]
- GIVEN a changed provider, plan, policy, platform, product set, output path, or unsafe derivation path
- WHEN request lowering runs
- THEN it MUST fail before any process starts

### Requirement: Local rooted execution

r[kiln.nix_build.execution] The adapter MUST execute the exact derivation through `nix-store --add-root … --indirect --realise` with a cleared environment, fixed `HOME`, `NIX_CONF_DIR`, and `NIX_CONFIG`, no remote builders or substituters, the Nix sandbox enabled, null stdin, bounded stdout/stderr, a deadline, process-group teardown, and optional cancellation.

#### Scenario: Exact invocation

r[kiln.nix_build.execution.scenario.invocation]
- GIVEN an admitted request
- WHEN dispatch runs
- THEN the invocation MUST use the pinned `nix-store` executable and exact derivation path
- AND it MUST create roots under the effect-specific private GC-root directory
- AND `NIX_CONFIG` MUST disable builders and substituters

#### Scenario: Deadline or cancellation is terminal

r[kiln.nix_build.execution.scenario.terminal]
- GIVEN a timed-out or cancelled process
- WHEN the output is interpreted
- THEN the observation MUST be `TimedOut` or `Cancelled`
- AND it MUST carry a terminal receipt

### Requirement: Completion admission

r[kiln.nix_build.completion] The adapter MUST admit success only when the native realisation contains exactly one root per expected output, every root is inside the effect-specific GC-root directory, and every root target equals the exact expected store path.

#### Scenario: Complete success

r[kiln.nix_build.completion.scenario.success]
- GIVEN zero exit and a rooted realisation with every expected output
- WHEN completion admission runs
- THEN the outcome MUST be `Success`
- AND every product MUST have its exact name, an artifact identity, and a store identity
- AND the observation MUST carry a receipt

#### Scenario: Missing or malformed native output

r[kiln.nix_build.completion.scenario.provider_error]
- GIVEN zero exit with malformed roots, a wrong root count, a missing output, a duplicate output, or a root target outside the expected path
- WHEN completion admission runs
- THEN the outcome MUST be `ProviderError` with no products and a terminal receipt

#### Scenario: Nonzero exit

r[kiln.nix_build.completion.scenario.failed]
- GIVEN a nonzero process status
- WHEN completion admission runs
- THEN the outcome MUST be `Failed` with no products and a terminal receipt

### Requirement: Output retention

r[kiln.nix_build.retention] The adapter MUST retain every successful output through a private effect-scoped GC-root directory and MUST remove those roots only through an explicit idempotent release after downstream durable handoff.

#### Scenario: Release after handoff

r[kiln.nix_build.retention.scenario.release]
- GIVEN a successful rooted build
- WHEN output release runs
- THEN every root in the effect-specific directory MUST be removed
- AND a repeated release MUST remain idempotent
- AND no observation-only restore may claim retained roots or old log bytes

### Requirement: Bounded logs

r[kiln.nix_build.logs] The adapter MUST bind one BLAKE3 identity to the bounded stderr log and MUST serve only bounded chunks for that exact effect and identity.

#### Scenario: Exact log read

r[kiln.nix_build.logs.scenario.read]
- GIVEN a recorded build and its log identity
- WHEN a bounded read is requested
- THEN the returned bytes MUST be the requested slice of the recorded stderr
- AND `end_of_log` MUST reflect the actual end

#### Scenario: Wrong identity or unavailable bytes

r[kiln.nix_build.logs.scenario.reject]
- GIVEN a wrong log identity, an unknown effect, a zero or oversized request, or an observation-only restore
- WHEN a log read runs
- THEN it MUST fail without returning unrelated bytes

### Requirement: Reconciliation

r[kiln.nix_build.reconciliation] The adapter MUST return the exact recorded observation for an equal repeated dispatch, reject a changed plan or request for the same effect, and make observe, reconcile, and observation-only restore exact.

#### Scenario: Duplicate dispatch is idempotent

r[kiln.nix_build.reconciliation.scenario.duplicate]
- GIVEN a recorded build
- WHEN the same plan is dispatched again
- THEN the recorded observation MUST be returned
- AND no second process MUST run

#### Scenario: Changed request is rejected

r[kiln.nix_build.reconciliation.scenario.conflict]
- GIVEN a recorded effect
- WHEN a changed plan or request is dispatched, reconciled, or restored
- THEN it MUST fail without rewriting the record

### Requirement: Bounded local-input preflight

r[kiln.nix_build.preflight] The shell MUST run a bounded `nix-store --realise --dry-run` preflight of the exact derivation before the rooted build and MUST fail without starting effects when the preflight fails, times out, is cancelled, or exceeds its output bound.

#### Scenario: Preflight succeeds

r[kiln.nix_build.preflight.scenario.success]
- GIVEN a mapped or explicitly bound derivation with local inputs
- WHEN the preflight runs
- THEN it MUST succeed without creating output roots
- AND the subsequent build MUST proceed

#### Scenario: Preflight fails

r[kiln.nix_build.preflight.scenario.failure]
- GIVEN a derivation whose local inputs are unavailable or whose preflight is rejected
- WHEN the preflight runs
- THEN it MUST return a failure before the rooted build
- AND no build output or observation MUST be admitted

### Requirement: Durable build state

r[kiln.nix_build.state] The operator shell MUST persist an intent before dispatch and a terminal observation after dispatch, MUST restore an exact observation without rebuilding, and MUST make root release idempotent and explicit in that durable state.

#### Scenario: State round trip

r[kiln.nix_build.state.scenario.round_trip]
- GIVEN a bound or task-derived profile and plan
- WHEN the state is stored and loaded
- THEN the profile source, plan, observation, and release state MUST round-trip
- AND a task-derived state MUST rehydrate to the same exact profile and plan

### Requirement: Discovery-task composition

r[kiln.nix_build.composition] The composition shell MUST map one discovery `TaskProposal` to an exact Nix profile and build plan, hash the task derivation before construction, bind the task output names and paths, expose doctor/build/from-task operator modes, and prove the mapping with fixture and live task builds.

#### Scenario: Task maps exactly

r[kiln.nix_build.composition.scenario.mapping]
- GIVEN a discovery task with a Nix derivation, output store paths, and build provider
- WHEN the task is mapped
- THEN the profile MUST use the exact derivation identity, plan identity, policy identity, and expected output paths
- AND the plan MUST use the task's sorted product names

#### Scenario: Task build is admitted

r[kiln.nix_build.composition.scenario.build]
- GIVEN a mapped task and build intent
- WHEN the composed build runs
- THEN the observation MUST be admitted and its roots MUST be retained
- AND release MUST remove them

#### Scenario: Real task E2E

r[kiln.nix_build.composition.scenario.e2e]
- GIVEN the pinned local Nix store and a discovery-shaped task
- WHEN the live composition test runs
- THEN it MUST realize the task derivation, admit the exact product, and release the rooted output

### Requirement: Nix build non-claims

r[kiln.nix_build.nonclaims] A passing Nix build observation MUST NOT claim build correctness, sandbox completeness, derivation provenance or source trust, output content semantics, cache truth, reproducibility, or release readiness.

#### Scenario: Observation scope

r[kiln.nix_build.nonclaims.scenario.scope]
- GIVEN a successful observation
- WHEN evidence is recorded
- THEN it MUST remain scoped to the exact executable, derivation, local store inputs, realisation roots, products, receipt, logs, and release state
