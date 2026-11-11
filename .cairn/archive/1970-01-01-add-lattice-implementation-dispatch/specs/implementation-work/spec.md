# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Bound Lattice implementation request dispatch

r[kiln.implementation.lattice_dispatch] Kiln MAY send one operator-supplied
Lattice workflow request only after it binds the reserved dispatch plan, attempt/
fence, exact repository/base and expected output; carries cancel-only controls;
uses an agent/local/custom graph; and validates the selected Lattice capability
profile. A disconnected acceptance MUST remain unknown for reconciliation. An
accepted response MUST match the request identity/idempotency linkage. Kiln MUST
NOT create the implementation workflow, choose a worker or treat transport
acceptance as implementation completion.

#### Scenario: Exact request dispatch

r[kiln.implementation.lattice_dispatch.scenario.admit]
- GIVEN a reserved attempt and exact operator-supplied Lattice request/profile
- WHEN dispatch runs
- THEN the request MUST be profile-validated and its acceptance linkage checked

#### Scenario: Mismatched request

r[kiln.implementation.lattice_dispatch.scenario.refused]
- GIVEN a request with a different effect/idempotency, attempt, source, control set or route
- WHEN admission runs
- THEN it MUST fail before transport
