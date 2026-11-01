# Ownership Evidence Specification

## Purpose

Defines named, exact-revision evidence attached to matching ownership rules.
Evidence is additive to owner approval and never grants merge authority.

## ADDED Requirements

### Requirement: Bounded evidence policy

r[kiln.review.evidence.policy] An ownership rule MAY declare
`required_evidence`. The closed kind set currently contains only `project_ci`
with a non-empty project identity. Policy admission MUST reject unknown kinds,
empty or untrimmed projects, duplicate projects within a rule, and more than
eight requirements per rule. Empty evidence lists MUST serialize as absent so
existing policies and journals retain identical bytes.

#### Scenario: Unknown kind is rejected
r[kiln.review.evidence.policy.scenario.unknown]
- GIVEN a rule with an evidence kind outside the closed set
- WHEN the policy is deserialized or admitted
- THEN it MUST be rejected.

#### Scenario: Duplicate or excessive evidence is rejected
r[kiln.review.evidence.policy.scenario.bounds]
- GIVEN a rule with duplicate project evidence or more than eight requirements
- WHEN the policy is admitted
- THEN it MUST be rejected as invalid.

### Requirement: Exact project CI evidence

r[kiln.review.evidence.project_ci] For every matching ownership rule and
`project_ci` requirement, the checklist MUST add one
`evidence:<rule>:project_ci:<project>` item. It MUST be satisfied only by a
full clean successful run for the exact submitted commit, the named project,
the change's repository and the project's current scope identity. Missing facts,
older revisions, another project or scope, non-clean runs and partial runs MUST
leave the item unsatisfied.

#### Scenario: Exact scoped evidence
r[kiln.review.evidence.project_ci.scenario.observed]
- GIVEN a matching rule requiring project `api` CI
- AND the current required scope for `api` is `api-scope`
- WHEN a full clean successful run exists for the exact commit, `api` and `api-scope`
- THEN the evidence item MUST be satisfied.

#### Scenario: Stale or partial evidence
r[kiln.review.evidence.project_ci.scenario.stale]
- GIVEN a matching evidence requirement
- WHEN the only run is for an older revision, another scope, is not full or is not clean
- THEN the evidence item MUST remain unsatisfied.

### Requirement: Evidence cannot replace approval

r[kiln.review.evidence.project_ci.authority] A satisfied evidence item MUST NOT
satisfy the rule's owner item, remove a veto or thread, prepare a candidate, or
grant merge authority.

#### Scenario: Evidence alone is insufficient
r[kiln.review.evidence.project_ci.scenario.approval]
- GIVEN a matching rule with satisfied evidence and no eligible owner approval
- WHEN the checklist is evaluated
- THEN the ownership checklist MUST remain incomplete.
