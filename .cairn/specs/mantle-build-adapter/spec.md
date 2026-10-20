# Mantle Build Adapter Specification

## Purpose

Defines the `mantle-build-adapter` capability.

## Requirements

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

### Requirement: Exact cohort refresh

r[kiln.mantle.cohort_refresh] The admitted Mantle cohort MUST name one exact published source revision and one exact executable identity, and a refresh MUST bind the revision, executable identity, vendored contract bytes, fixture record, Nix cohort check, and documentation together.

#### Scenario: Refresh binds a consistent cohort

- GIVEN a published Mantle revision whose producer contract bytes match the admitted wire contract
- WHEN the cohort is refreshed through an admitted change
- THEN the profile, vendored source record, fixture record, Nix cohort check, and documentation MUST name the same revision and executable identity.

#### Scenario: Partial refresh is rejected

- GIVEN a profile, fixture record, or Nix cohort value that still names a different revision or executable identity
- WHEN profile admission or the cohort check runs
- THEN it MUST reject the mixed cohort without admitting the producer.

### Requirement: Cohorts change only through an admitted change

r[kiln.mantle.cohort_refresh.contract] A refreshed cohort MUST preserve the admitted request, observation, receipt, and native report schemas unless the same change also extends the adapter contract and its positive and negative fixtures. The executable identity MUST be observed from a clean export of the selected revision, not from a mutable producer working tree. Revision, executable identity, fixture revision, and importer pin MUST agree; any disagreement MUST fail validation or the Nix cohort check. Historical release, publication, and verification receipts MUST retain their original meaning and MUST NOT be rewritten to describe the refreshed cohort.

#### Scenario: Refreshed executable identity is observed from a clean export

r[kiln.mantle.cohort_refresh.identity]
- GIVEN a selected producer revision
- WHEN the cohort refresh records its executable identity
- THEN the identity MUST come from a clean export of that revision
- AND it MUST NOT come from a mutable producer working tree

#### Scenario: Cohort identities agree

r[kiln.mantle.cohort_refresh.drift]
- GIVEN a refreshed profile, fixture record, importer pin, or Nix cohort check
- WHEN cohort admission or the Nix cohort check runs
- THEN the revision and executable identity MUST agree across every record
- AND any disagreement MUST fail validation

#### Scenario: Historical receipts keep their original meaning

r[kiln.mantle.cohort_refresh.records]
- GIVEN a historical release, publication, or verification receipt
- WHEN the cohort is refreshed
- THEN that receipt MUST retain its original meaning
- AND it MUST NOT be rewritten to describe the refreshed cohort
