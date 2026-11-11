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

### Requirement: Guarded merge-commit landing

r[kiln.github.landing.merge] Kiln MAY land one admitted landing observation with
a merge-commit request bound to the exact PR head SHA. It MUST first admit the
full PR/check/review/base observation. After the request it MUST read the PR and
target base ref back and return a receipt only when the PR is merged at the exact
head/base and the base head equals the merge commit. A timeout or ambiguous
response MUST reconcile by read-back; an unobservable merge commit MUST remain
unknown. The route MUST NOT close issues, mutate source status or claim atomic
base protection or merge-queue qualification.

#### Scenario: Merge lands and reads back

r[kiln.github.landing.merge.scenario.landed]
- GIVEN an exact ready observation
- WHEN the merge request succeeds
- THEN the receipt MUST bind the candidate head, merge commit, base and base head

#### Scenario: Merge does not land

r[kiln.github.landing.merge.scenario.refused]
- GIVEN a refused merge or no observable merged state
- WHEN landing reconciliation runs
- THEN it MUST fail or remain unknown without claiming a landed result

### Requirement: Reconcile merge ancestry after base advancement

r[kiln.github.landing.merge_ancestry] Landing read-back MUST accept a merge
result when the current base head equals the merge commit or GitHub's compare
read-back reports the base head ahead of or identical to the merge commit with
that merge commit as the merge base. A missing merge commit, divergent base or
unrelated merge base MUST remain unknown/refused. This ancestry check MUST NOT be
described as atomic merge-queue or base-protection qualification.

#### Scenario: Base advanced after landing

r[kiln.github.landing.merge_ancestry.scenario.advanced]
- GIVEN a merged candidate and a later base head
- WHEN compare proves the merge commit is the merge base and the base is ahead
- THEN the existing landing receipt MUST be recovered without another merge
