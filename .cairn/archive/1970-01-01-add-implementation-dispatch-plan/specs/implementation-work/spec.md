# Implementation Work Specification

## ADDED Requirements

### Requirement: Bounded implementation dispatch plan

r[kiln.implementation.dispatch_plan] A dispatch plan MUST bind the exact work
identity and revision, attempt identity and fence, repository, frozen base,
isolated workspace identity, provider profile, admitted paths and effects,
cancellation-only controls, positive deadline, token and invocation budgets, and
expected output identity. Admission MUST refuse any mismatch, any path or effect
the work did not admit, a deadline beyond the attempt budget, non-cancel
controls, network access, credentials, or budgets over their named bounds. The
canonical plan identity MUST be deterministic over these facts, and reservation
MUST use it.

#### Scenario: Exact plan is admitted and reserved

r[kiln.implementation.dispatch_plan.scenario.admit]
- GIVEN current work and a claimed attempt
- WHEN an exact bounded plan is admitted and reserved
- THEN the reservation MUST carry the deterministic plan identity

#### Scenario: Broader plan is refused

r[kiln.implementation.dispatch_plan.scenario.refused]
- GIVEN a plan with a different binding, broader path/effect, pause/resume control, network or credentials, an excessive deadline, or an empty/oversized budget
- WHEN admission runs
- THEN it MUST fail before reservation or transport
