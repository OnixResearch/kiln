# Implementation Work Specification

## ADDED Requirements

### Requirement: Exact resolved base participates in work identity

r[kiln.implementation.base] Admitted work MAY carry one exact resolved target/base
revision. Absence MUST remain explicit and MUST NOT be inferred from a symbolic
reference. The base MUST participate in the canonical work identity, so a base
change produces a superseding version. A claim MUST freeze the base in the
attempt, and later work edits MUST NOT mutate the frozen attempt.

#### Scenario: Base changes identity

r[kiln.implementation.base.scenario.identity]
- GIVEN otherwise equal work with and without an exact base revision
- WHEN identities are computed
- THEN the identities MUST differ

#### Scenario: Claim freezes base

r[kiln.implementation.base.scenario.freeze]
- GIVEN work with an exact base
- WHEN an attempt is claimed and the work base later changes
- THEN the attempt MUST retain the original base and work revision
