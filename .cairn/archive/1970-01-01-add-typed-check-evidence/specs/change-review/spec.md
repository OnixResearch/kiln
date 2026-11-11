# Change Review Specification

## ADDED Requirements

### Requirement: Typed check evidence

r[kiln.review.checks] CI evidence MUST expose named check results bound to the
exact reviewed commit, with a known check kind, an outcome, and an optional
artifact digest. A configured required check MUST be satisfied only by a
passing check of the required kind and optional name on the exact winning CI
run. Unknown or duplicate requirements MUST fail policy admission. Typed checks
are evidence and MUST NOT grant gate, merge or release authority by themselves.

#### Scenario: Required check passes

- GIVEN a policy requiring a `property` check named `property-suite`
- WHEN the exact winning CI run retains a passing check of that kind and name
- THEN the `checks` checklist item is satisfied.

#### Scenario: Wrong kind or commit

- GIVEN the same requirement and a check that is failing, has another kind, or
  belongs to another commit
- WHEN the checklist is evaluated
- THEN the requirement remains unmet.

#### Scenario: Invalid policy

- GIVEN an unknown kind, duplicate kind/name pair, empty name, or oversized
  requirement list
- WHEN policy admission runs
- THEN it MUST reject without changing review authority.
