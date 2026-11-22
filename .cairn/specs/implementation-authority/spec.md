# Implementation Authority Specification

## Purpose

Defines the `implementation-authority` capability.

## Requirements

### Requirement: Closed protected-operation coverage

r[kiln.implementation.authority.coverage] Kiln MUST retain a closed coverage map
from every protected path in the selected implementation profile to exactly one
reviewed Basalt operation, one enforcement owner, one effect class and whether a
candidate identity is bound. An operation or protected path without an entry MUST
deny, and no operation MUST imply another.

#### Scenario: Every protected operation is mapped once

r[kiln.implementation.authority.coverage.scenario.complete]
- GIVEN the closed claim, renewal, dispatch, submission, verification, cancellation, publication, integration and lifecycle operations
- WHEN the coverage map is inspected
- THEN every operation MUST appear exactly once with a unique owner and the reviewed candidate rule
- AND the map MUST contain no duplicate Basalt operation id

#### Scenario: Unmapped path denies

r[kiln.implementation.authority.coverage.scenario.unmapped]
- GIVEN a protected path with no coverage entry
- WHEN authority is requested for it
- THEN the request MUST deny before any effect

### Requirement: Exact request binding

r[kiln.implementation.authority.binding] Each authority request MUST bind the
holder, deployment audience, project, work and version, attempt and fence,
authority generation, canonical payload digest, target and the applicable
candidate identity. The caller MUST NOT supply a trusted current-state
observation, and substituting any bound field MUST invalidate the request.

#### Scenario: Substituted binding denies

r[kiln.implementation.authority.binding.scenario.substitution]
- GIVEN a verified request for one exact operation
- WHEN any bound field differs
- THEN authorization MUST deny before credential resolution or effect

#### Scenario: Candidate is bound exactly where a mutation needs one

r[kiln.implementation.authority.binding.scenario.candidate]
- GIVEN submission, verification, publication, integration and lifecycle bind a candidate
- WHEN the candidate identity is absent for one of them, or present for claim, dispatch or cancellation
- THEN authorization MUST deny

### Requirement: Verified live authority before protected effects

r[kiln.implementation.authority.enforcement] A protected read, disclosure,
mutation or external effect MUST require current verified authority for that exact
request. A caller boolean, role label, serialized receipt, provider capability
advertisement or historical approval MUST NOT manufacture authority. Credentials
MUST be resolved only after authorization succeeds.

#### Scenario: Missing authority denies before effect

r[kiln.implementation.authority.enforcement.scenario.absent]
- GIVEN a claim, dispatch, submission, verification, publication, integration or lifecycle request without a verified session
- WHEN the owning boundary evaluates it
- THEN it MUST deny before any mutation, provider call or credential resolution

#### Scenario: Retained allow evidence is not authority

r[kiln.implementation.authority.enforcement.scenario.replay]
- GIVEN a serialized request, redacted receipt or prior allowed decision
- WHEN it is presented for a new effect
- THEN it MUST NOT authorize that effect

### Requirement: Currentness blocks superseded attempts

r[kiln.implementation.authority.currentness] Revalidation MUST check signatures,
proofs, time, revocations, policy and deployment generation before each protected
effect without consuming replay twice. Cancellation, supersession, lease expiry
and fence replacement MUST block later effects for the affected attempt even when
another still-valid proof is presented. Unavailable required currentness MUST fail
closed for new effects; independently authorized reconciliation and cleanup MUST
remain possible.

#### Scenario: Revoked or replaced attempt is blocked

r[kiln.implementation.authority.currentness.scenario.blocked]
- GIVEN a cancelled, superseded or revoked attempt
- WHEN a later effect is requested with any proof
- THEN the effect MUST deny
- AND retained evidence and authorized cleanup MUST remain available

#### Scenario: Unavailable currentness fails closed

r[kiln.implementation.authority.currentness.scenario.unavailable]
- GIVEN a required revocation or authority source that cannot be read
- WHEN a new effect is requested
- THEN it MUST deny rather than assume current authority

### Requirement: Execution authority is not verification or publication authority

r[kiln.implementation.authority.separation] Claim, dispatch and candidate
submission authority MUST NOT admit verification acceptance, publication,
integration or native lifecycle completion. Each protected operation MUST be
authorized by its own grant, and an authorization decision MUST NOT create a
passing check or discharge a Cairn obligation.

#### Scenario: Dispatch grant cannot publish

r[kiln.implementation.authority.separation.scenario.widen]
- GIVEN a session admitted for dispatch or submission
- WHEN publication, integration or lifecycle completion is requested
- THEN it MUST deny

### Requirement: Explicit local-operator compatibility

r[kiln.implementation.authority.compatibility] A local-operator compatibility
profile MAY exist only when explicitly selected and MUST record its provenance in
the decision. It MUST NOT authorize projection-enabled routes, MUST NOT widen by
role alone, and MUST NOT provide a fallback for missing authority.

#### Scenario: Role label alone denies

r[kiln.implementation.authority.compatibility.scenario.role]
- GIVEN an operator or worker role label without a verified session or the selected local-operator profile
- WHEN a protected operation is requested
- THEN it MUST deny

### Requirement: One UCAN cohort and recorded Basalt import

r[kiln.implementation.authority.cohort] Kiln MUST serve observation authentication
and implementation authority from one vendored UCAN snapshot and MUST import the
Basalt adapter through a reproducible repository-owned transform that records the
source revision, archive identity and path rewrites.

#### Scenario: Import is reproducible

r[kiln.implementation.authority.cohort.scenario.import]
- GIVEN the recorded Basalt import
- WHEN the import script runs again
- THEN it MUST produce the same vendored tree
- AND the manifest rewrite MUST be recorded, not inferred
