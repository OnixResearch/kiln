# GitHub Landing Specification

## Purpose

Defines the `github-landing` capability.

## Requirements

### Requirement: Read-only landing observation

r[kiln.github.landing] Kiln MAY read one GitHub pull request and its bounded
check-run page to produce landing evidence. The observation MUST bind repository,
PR number, exact head commit, base ref and operation identity, and MUST require
the PR to be open, unmerged, mergeable, at the exact head/base, with every
required check unique, completed and successful on that head. Missing, stale,
duplicate, failed or incompletely paginated checks MUST be refused. The observer
MUST NOT merge, approve, update or otherwise mutate remote state.

#### Scenario: Exact ready observation

r[kiln.github.landing.scenario.ready]
- GIVEN an open mergeable PR at the exact head/base with all required successful checks
- WHEN the observer reads it
- THEN bounded landing evidence MUST be returned

#### Scenario: Stale or incomplete observation

r[kiln.github.landing.scenario.refused]
- GIVEN a moved head, closed/merged PR, missing/failed/duplicate check or incomplete check pagination
- WHEN observation admission runs
- THEN it MUST refuse to produce landing evidence

### Requirement: Independent landing approvals

r[kiln.github.landing.approvals] A landing plan MUST declare a bounded required
approval count. Observation admission MUST use the latest review state per
actor, refuse any latest `CHANGES_REQUESTED`, exclude the pull-request author
from the approval count, and require at least the declared number of distinct
independent `APPROVED` actors. Missing or insufficient approvals MUST refuse
landing evidence.

#### Scenario: Distinct independent approvals

r[kiln.github.landing.approvals.scenario.distinct]
- GIVEN the latest review per actor with no changes requested
- WHEN admission runs
- THEN only distinct non-author approvals MUST count toward the requirement
- AND insufficient approvals MUST be refused

### Requirement: Independent landing approvals

r[kiln.github.landing.approvals] A landing plan MUST declare a bounded required
approval count. Observation admission MUST use the latest review state per
actor, refuse any latest `CHANGES_REQUESTED`, exclude the pull-request author
from the approval count, and require at least the declared number of distinct
independent `APPROVED` actors. Missing or insufficient approvals MUST refuse
landing evidence.

#### Scenario: Distinct independent approvals

r[kiln.github.landing.approvals.scenario.distinct]
- GIVEN the latest review per actor with no changes requested
- WHEN admission runs
- THEN only distinct non-author approvals MUST count toward the requirement
- AND insufficient approvals MUST be refused

### Requirement: Exact base-head landing evidence

r[kiln.github.landing.base] A landing plan MUST declare the exact expected base
branch head. Observation MUST read the current base branch head and require it to
equal that commit. A missing, malformed or moved base head MUST refuse landing
evidence. The base observation is read-only and does not prove atomic base
stability during a later merge.

#### Scenario: Exact base head

r[kiln.github.landing.base.scenario.exact]
- GIVEN the base branch is at the expected commit
- WHEN landing evidence is observed
- THEN the base head MUST match and be retained in the evidence

#### Scenario: Moved base head

r[kiln.github.landing.base.scenario.moved]
- GIVEN the base branch head differs from the plan
- WHEN admission runs
- THEN landing evidence MUST be refused
