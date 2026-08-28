# Aspen Host Bridge Specification Delta

## Added Requirements

### Requirement: Paced bounded Lattice observation

r[kiln.aspen_host.lattice_poll_pacing] The production Aspen host MUST use one explicit positive interval between non-terminal Lattice observations while preserving the existing finite poll bound.

#### Scenario: Delayed workflow reaches a terminal observation

r[kiln.aspen_host.lattice_poll_pacing.accepted]
- GIVEN Lattice accepts one workflow and first returns a non-terminal observation
- AND the host has a positive observation interval and finite poll count
- WHEN Lattice reaches a terminal observation before that count is exhausted
- THEN the host MUST wait between observe calls
- AND it MUST persist the exact terminal observation without provider redispatch

#### Scenario: Pacing is invalid or exhausted

r[kiln.aspen_host.lattice_poll_pacing.rejected]
- GIVEN the observation interval is missing, duplicated, zero, malformed, or the finite poll count expires
- WHEN host admission or workflow routing runs
- THEN invalid admission MUST fail before the host binds its socket
- AND bound exhaustion MUST remain `Unknown`
- AND recovery MUST use `RecoverDispatch` and observation without normal redispatch
