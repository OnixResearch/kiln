# Nix Build Specification Delta

## Added Requirements

### Requirement: Exact Nix build cohort

r[kiln.nix_build.cohort] The Nix build adapter MUST admit one pinned Nix version, executable path and BLAKE3 identity, request/observation/receipt/native schemas, command, platform, Bounded Exec revision, limits, and non-claims before any process starts.

#### Scenario: Pinned cohort is admitted

r[kiln.nix_build.cohort.scenario.pinned]
- GIVEN the pinned executable and derivation identities
- WHEN profile admission runs
- THEN the profile MUST be admitted
- AND the provider MUST declare the build role, dispatch/observe/cancel/read-logs/reconcile operations, and all terminal outcome classes

#### Scenario: Cohort or limit drift fails closed

r[kiln.nix_build.cohort.scenario.drift]
- GIVEN a changed executable identity, derivation identity, version, capability set, product order, or inconsistent limit
- WHEN profile admission or adapter construction runs
- THEN it MUST fail before any build process starts

### Requirement: Exact derivation request

r[kiln.nix_build.request] The adapter MUST require the build plan's provider profile, plan identity, policy identity, platform, and sorted products to equal the exact admitted runtime binding, and MUST bind one safe absolute `.drv` directly under the admitted store.

#### Scenario: Exact plan is lowered

r[kiln.nix_build.request.scenario.exact]
- GIVEN a plan that matches the profile and one safe derivation child
- WHEN request lowering runs
- THEN the request MUST contain the exact derivation path and required products

#### Scenario: Plan or derivation mismatch fails before execution

r[kiln.nix_build.request.scenario.mismatch]
- GIVEN a changed provider, plan, policy, platform, product set, or unsafe derivation path
- WHEN request lowering runs
- THEN it MUST fail before any process starts

### Requirement: Local bounded execution

r[kiln.nix_build.execution] The adapter MUST execute the exact derivation through Bounded Exec with a cleared environment, fixed `HOME` and `NIX_CONFIG`, no remote builders or substituters, null stdin, bounded stdout/stderr, a deadline, process-group teardown, and optional cancellation.

#### Scenario: Exact invocation

r[kiln.nix_build.execution.scenario.invocation]
- GIVEN an admitted request
- WHEN dispatch runs
- THEN the invocation MUST use the pinned executable and exact `import <drv>` expression
- AND `NIX_CONFIG` MUST disable builders and substituters
- AND the current directory and `HOME` MUST be the admitted home directory

#### Scenario: Deadline or cancellation is terminal

r[kiln.nix_build.execution.scenario.terminal]
- GIVEN a timed-out or cancelled process
- WHEN the output is interpreted
- THEN the observation MUST be `TimedOut` or `Cancelled`
- AND it MUST carry a terminal receipt

### Requirement: Completion admission

r[kiln.nix_build.completion] The adapter MUST admit success only when the native JSON report contains exactly one result, the exact admitted `drvPath`, and every required product exactly once as a direct child of the admitted store, and MUST map every other native condition to an explicit terminal outcome.

#### Scenario: Complete success

r[kiln.nix_build.completion.scenario.success]
- GIVEN zero exit and a report with every required output
- WHEN completion admission runs
- THEN the outcome MUST be `Success`
- AND every product MUST have an artifact identity, store identity, and the plan product name
- AND the observation MUST carry a receipt

#### Scenario: Missing or malformed native output

r[kiln.nix_build.completion.scenario.provider_error]
- GIVEN zero exit with malformed JSON, a wrong derivation, a missing product, or an output outside the store
- WHEN completion admission runs
- THEN the outcome MUST be `ProviderError` with no products and a terminal receipt

#### Scenario: Nonzero exit

r[kiln.nix_build.completion.scenario.failed]
- GIVEN a nonzero process status
- WHEN completion admission runs
- THEN the outcome MUST be `Failed` with no products and a terminal receipt

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

### Requirement: Nix build non-claims

r[kiln.nix_build.nonclaims] A passing Nix build observation MUST NOT claim build correctness, sandbox completeness, derivation provenance or source trust, output content semantics, cache truth, reproducibility, or release readiness.

#### Scenario: Observation scope

r[kiln.nix_build.nonclaims.scenario.scope]
- GIVEN a successful observation
- WHEN evidence is recorded
- THEN it MUST remain scoped to the exact executable, derivation, local store inputs, native report, products, receipt, and logs
