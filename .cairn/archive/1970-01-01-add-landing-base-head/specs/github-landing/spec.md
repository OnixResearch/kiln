# GitHub Landing Delta Specification

## ADDED Requirements

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
