# Mantle Build Adapter Delta Specification

## ADDED Requirements

### Requirement: Versioned Mantle build profile
r[kiln.mantle.profile] Kiln MUST admit Mantle only through a typed profile bound to one exact published contract and capability set.

#### Scenario: Contract matches
r[kiln.mantle.profile.scenario.match]
- GIVEN a complete profile for the pinned Mantle cohort
- WHEN provider admission runs
- THEN Kiln MAY make the declared build capabilities available.

#### Scenario: Contract drifts
r[kiln.mantle.profile.scenario.drift]
- GIVEN a profile with a mismatched schema, revision, digest cohort, or capability
- WHEN provider admission runs
- THEN Kiln MUST reject the provider before dispatch.

### Requirement: Exact Mantle request linkage
r[kiln.mantle.request] Every Mantle request MUST bind one Kiln effect, attempt, candidate or source, pipeline revision, plan, outputs, policy, and idempotency key.

#### Scenario: Request is complete
r[kiln.mantle.request.scenario.valid]
- GIVEN one admitted Kiln build effect and selected Mantle profile
- WHEN pure request planning runs
- THEN it MUST produce one bounded request with exact linkage.

#### Scenario: Candidate identity differs
r[kiln.mantle.request.scenario.substitution]
- GIVEN a request whose candidate differs from the admitted effect
- WHEN request admission runs
- THEN Kiln MUST reject it before Mantle dispatch.

### Requirement: Mantle completion admission
r[kiln.mantle.completion] A Mantle observation MUST pass request, identity, product, receipt, bound, and outcome admission before canonical completion.

#### Scenario: Required products are present
r[kiln.mantle.completion.scenario.success]
- GIVEN a linked Mantle success with every required admitted product
- WHEN completion admission runs
- THEN Kiln MAY record the corresponding terminal outcome.

#### Scenario: Cache hit lacks a product
r[kiln.mantle.completion.scenario.cache]
- GIVEN a Mantle cache observation without one required product
- WHEN completion admission runs
- THEN Kiln MUST reject canonical success.

### Requirement: Mantle reconciliation
r[kiln.mantle.reconciliation] Ambiguous dispatch and cancellation results MUST remain unknown until exact native reconciliation supplies an admitted observation.

#### Scenario: Connection fails after acceptance
r[kiln.mantle.reconciliation.scenario.disconnect]
- GIVEN a dispatched request whose connection fails after provider acceptance
- WHEN outcome classification runs
- THEN Kiln MUST record unknown and retain the native reconciliation identity.

#### Scenario: Reconciliation names another request
r[kiln.mantle.reconciliation.scenario.mismatch]
- GIVEN a reconciliation result linked to another effect or attempt
- WHEN observation admission runs
- THEN Kiln MUST reject the result without changing state.

### Requirement: Mantle adapter conformance
r[kiln.mantle.conformance] Maintained conformance MUST include pinned producer fixtures and an independent Kiln consumer with positive and negative cases.

#### Scenario: Adapter has handwritten success only
r[kiln.mantle.conformance.scenario.external]
- GIVEN a candidate without pinned external compatibility evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Mantle adapter non-claims
r[kiln.mantle.nonclaims] Passing adapter checks MUST NOT claim build correctness, sandbox completeness, cache truth, product semantics, or release readiness.

#### Scenario: Build observation passes
r[kiln.mantle.nonclaims.scenario.scope]
- GIVEN a Mantle observation that passes completion admission
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the exact request, products, contract, policy, and observations.
