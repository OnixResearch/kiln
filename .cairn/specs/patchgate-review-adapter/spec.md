# Patchgate Review Adapter Specification

## Purpose

Defines the `patchgate-review-adapter` capability.

## Requirements

### Requirement: Explicit isolated review profile
r[kiln.patchgate.profile] Kiln MUST admit only the declared Patchgate revision/report cohort, explicit executable identity, collaboration-observation capabilities, environment and positive named resource limits.

#### Scenario: Exact profile
r[kiln.patchgate.profile.scenario.match]
- GIVEN an exact installed executable and supported isolated profile
- WHEN provider admission runs
- THEN Kiln MAY execute only isolated selected review.

#### Scenario: Unsupported authority or executable
r[kiln.patchgate.profile.scenario.drift]
- GIVEN executable drift, an unsupported report version or a mutation capability
- WHEN provider admission runs
- THEN Kiln MUST refuse before executing review.

### Requirement: Exact review applicability
r[kiln.patchgate.request] A review request MUST bind an exact Kiln candidate set, pipeline, policy, effect, idempotency key, provider profile, native initial channel states, trusted configuration and selected changes.

#### Scenario: Complete request
r[kiln.patchgate.request.scenario.exact]
- GIVEN one admitted candidate and explicit native binding
- WHEN planning runs
- THEN the request identity MUST deterministically bind every applicability fact.

#### Scenario: Native state or selection changes
r[kiln.patchgate.request.scenario.stale]
- GIVEN evidence for another initial channel state, configuration or selected change
- WHEN admission runs
- THEN Kiln MUST reject applicability rather than reuse that result.

### Requirement: Complete native result admission
r[kiln.patchgate.result] Kiln MUST preserve native review decisions, prerequisite facts, per-record reviewed main states and verification/provenance bytes without interpreting source-nonmutating simulation as acceptance authority.

#### Scenario: Complete isolated decision
r[kiln.patchgate.result.scenario.complete]
- GIVEN a complete isolated report with exact initial bindings
- WHEN native result admission runs
- THEN Kiln MUST preserve accepted, rejected, escalated, blocked and waiting outcomes separately and retain the original report identity.

#### Scenario: Incomplete or substituted report
r[kiln.patchgate.result.scenario.invalid]
- GIVEN malformed, duplicate, missing, unrelated, interrupted or authority-promoting output
- WHEN native result admission runs
- THEN Kiln MUST NOT produce an approving complete observation.

### Requirement: Bounded review execution
r[kiln.patchgate.execution] The shell MUST use fixed isolated argv, explicit cleared environment, bounded output, deadline, cancellation and owned process-group teardown; infrastructure failures MUST remain distinct from review decisions.

#### Scenario: Deadline or cancellation
r[kiln.patchgate.execution.scenario.stop]
- GIVEN an active isolated review
- WHEN the deadline expires or cancellation is requested
- THEN the shell MUST stop its owned child group and MUST NOT claim review acceptance.

#### Scenario: Retained result reconciliation
r[kiln.patchgate.execution.scenario.reconcile]
- GIVEN retained evidence from an exact completed request
- WHEN reconciliation runs
- THEN it MUST re-admit that evidence without dispatching another model review.

### Requirement: Real producer conformance and non-claims
r[kiln.patchgate.conformance] Maintained validation MUST pair deterministic consumer positives and negatives with a repeatable real installed Patchgate/Pijul source-nonmutation rail.

#### Scenario: Source stays unchanged
r[kiln.patchgate.conformance.scenario.native]
- GIVEN real recorded accepted, rejected and escalated fixture changes
- WHEN isolated review runs through the operator surface
- THEN independently observed original channels, work files and gate audit state MUST stay unchanged.

### Requirement: Review evidence non-claims

r[kiln.patchgate.nonclaims] Review evidence MUST NOT claim source mutation permission, backend-weight verification, a reusable final merged snapshot, source correctness, deployment or release authority; default builds MUST NOT acquire private Patchgate sources.

#### Scenario: Simulated acceptance
r[kiln.patchgate.nonclaims.scenario.authority]
- GIVEN a native isolated acceptance
- WHEN Kiln emits the observation
- THEN it MUST declare no acceptance authority and retain the original evidence limitations.

### Requirement: Retained review receipts in the live workspace
r[kiln.dashboard.reviews.view]
The live workspace MUST provide a Reviews page over an operator-declared receipt directory and a Reviews tab over a run's retained reports, each selecting only `kiln.patchgate.review.v1` receipts and renders the preserved request, candidate, policy, binding, report and effect identities, the normalized per-change decisions with their distinct states, the producer's isolation and acceptance-authority claims, and the verbatim native report, without granting review, merge, gate or release authority.

#### Scenario: A retained receipt is inspected
r[kiln.dashboard.reviews.view.scenario.retained]
- GIVEN a run with one retained review report whose document schema is `kiln.patchgate.review.v1`
- WHEN an admitted reader opens the Reviews view
- THEN every recorded decision MUST remain distinct and inspectable, acceptance authority MUST read as none unless the producer claimed otherwise, and the native report MUST be shown verbatim

#### Scenario: No receipt is retained
r[kiln.dashboard.reviews.view.scenario.absent]
- GIVEN no configured receipt directory, a run with no retained Patchgate receipt, or a directory entry that is not a valid receipt
- WHEN an admitted reader opens the Reviews page or tab
- THEN the view MUST report the absence or skip reason explicitly rather than implying a clean review, and unrelated reports MUST remain under the existing structured-report view
