# Lattice Issue Closeout

## ADDED Requirements

### Requirement: Pin the CI handler independently of the MIT contract import

r[kiln.lattice.runtime_pin] Operator and Radicle workflow documentation MUST identify Lattice runtime 85a38b6a4bceeae4bb2057cf157e21752100f1b5 separately from the MIT contract import 70496e67c7fd4a8b05914161a8e09de2759bebc8. Historical Aspen observations MUST NOT qualify the newer handler.

#### Scenario: Maintained cohort

- GIVEN the maintained Lattice CI profile
- WHEN an operator selects the handler and controls
- THEN the documented runtime and supported operations match the admitted cohort.

#### Scenario: Unsupported assumption

- GIVEN an old handler or an unsupported control
- WHEN the operator or adapter checks compatibility
- THEN the old contract is not represented as runtime qualification and unsupported effects are not dispatched.
