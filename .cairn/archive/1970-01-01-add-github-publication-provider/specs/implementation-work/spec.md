# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Publication target base branch

r[kiln.implementation.publication_target] A publication plan MUST bind a
nonempty, bounded, non-escaping target base branch in addition to the frozen
base revision. An invalid target branch MUST fail preparation before any remote
effect.

#### Scenario: Invalid target branch

r[kiln.implementation.publication_target.scenario.refused]
- GIVEN a publication plan with an empty, absolute or escaping target branch
- WHEN preparation runs
- THEN it MUST fail
