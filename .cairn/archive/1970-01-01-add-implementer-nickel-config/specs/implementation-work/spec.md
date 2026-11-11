# Implementation Work Specification

## ADDED Requirements

### Requirement: Typed implementation policy and enrollment configuration

r[kiln.implementation.nickel_config] Implementation intake policy and enrollment
MUST be authorable as typed Nickel that exports the JSON consumed by
`kiln-implementer`. The policy builder MUST bound acceptance obligations,
repository-relative paths, effect identifiers, a 1–10 attempt budget, a positive
per-attempt time, a known terminal policy, and an explicit priority. The
enrollment builder MUST require unique, non-empty repository and actor strings.
Invalid inputs MUST fail export.

#### Scenario: Valid configuration exports

r[kiln.implementation.nickel_config.scenario.export]
- GIVEN a complete policy and enrollment input
- WHEN Nickel exports them
- THEN the JSON MUST match the fields consumed by the operator command

#### Scenario: Invalid configuration is rejected

r[kiln.implementation.nickel_config.scenario.reject]
- GIVEN an absolute permitted path, an empty actor list, or an out-of-range budget
- WHEN Nickel exports the configuration
- THEN export MUST fail
