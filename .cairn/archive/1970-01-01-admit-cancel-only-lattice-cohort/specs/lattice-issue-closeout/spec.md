# Lattice Issue Closeout

## ADDED Requirements

### Requirement: Match admitted controls to the real Lattice handler

r[kiln.lattice.cancel_only] The maintained Lattice cohort MUST admit cancel and refuse pause and resume before transport. Provider operations, Nickel profiles and consumer plans MUST agree. Paused observation mapping MUST remain available for a future producer cohort.

#### Scenario: Maintained cohort

- GIVEN the maintained Lattice CI profile
- WHEN an operator selects the handler and controls
- THEN the documented runtime and supported operations match the admitted cohort.

#### Scenario: Unsupported assumption

- GIVEN an old handler or an unsupported control
- WHEN the operator or adapter checks compatibility
- THEN the old contract is not represented as runtime qualification and unsupported effects are not dispatched.
