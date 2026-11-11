# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Exact dispatch base

r[kiln.implementation.dispatch_base] GitHub and Cairn intake policies MAY supply
an exact resolved base revision; when supplied it MUST be carried unchanged into
the admitted work version. A dispatch plan MUST require a present base and MUST
match both the work base and the base frozen on the attempt. A plan with an
absent or different base MUST fail before reservation or transport.

#### Scenario: Policy supplies the base

r[kiln.implementation.dispatch_base.scenario.supplied]
- GIVEN an intake policy with an exact base
- WHEN an issue or Cairn unit is admitted
- THEN the work version MUST carry the same base identity

#### Scenario: Dispatch requires the exact base

r[kiln.implementation.dispatch_base.scenario.required]
- GIVEN work or an attempt without a base, or a plan with a different base
- WHEN dispatch-plan admission runs
- THEN it MUST fail before transport
