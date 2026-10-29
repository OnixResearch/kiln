# Network Authenticated Web Access Specification

## Purpose

Defines the `network-authenticated-web-access` capability.

## Requirements

### Requirement: Discover authorized network access without a browser token

r[kiln.web.network.bootstrap]

The web workspace SHALL admit a network session only after a bounded same-origin project request without Authorization returns a valid successful response. A denied or stale discovery SHALL NOT admit a workspace or override a later manual session.

#### Scenario: Authorized tailnet entry

- GIVEN the operator's origin-checked tailnet proxy supplies the backend credential
- WHEN a fresh browser opens the tailnet workspace with no saved credentials
- THEN the existing projects, runs and live observations are available without entering or receiving a bearer token

#### Scenario: Localhost remains protected

- GIVEN the direct localhost service still requires its original operator credential
- WHEN discovery receives 401
- THEN the page retains manual bearer login and no private workspace is admitted

### Requirement: Preserve origin, role and durable-state boundaries

r[kiln.web.network.boundaries]

The tailnet shell SHALL retain exact public Host/Origin admission and validated backend TLS, SHALL keep credential material server-side, and SHALL NOT replace service/worker profiles or canonical run state. Manual sessions and worker role checks SHALL remain unchanged.

#### Scenario: Invalid or missing write origin

- GIVEN the tailnet proxy authorizes reachable peers
- WHEN a write has a foreign or absent Origin
- THEN the write is rejected without an external effect

#### Scenario: Network page restoration

- GIVEN a network-authorized page has been left
- WHEN it is restored
- THEN prior private state is cleared and authorization is discovered afresh rather than reviving a bearer or trusting a prior snapshot
