# GitHub Landing Delta Specification

## ADDED Requirements

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
