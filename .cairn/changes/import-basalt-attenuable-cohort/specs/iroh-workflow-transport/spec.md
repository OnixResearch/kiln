# Iroh workflow transport delta

## MODIFIED Requirements

### Requirement: Versioned execution transport profile
r[kiln.iroh.workflow.profile] Kiln MUST pin Iroh 1.1.0 and ALPN `kiln.workflow.v1`, admit explicit stable identities, peer/relay policy, native Lattice contract profile, private local socket and positive finite frame/log/concurrency/deadline bounds before opening an execution connection. Core CI crates MUST remain provider-neutral.

#### Scenario: Admitted profile
r[kiln.iroh.workflow.profile.scenario.valid]
- GIVEN a complete profile matching the pinned cohort and bounded native contract
- WHEN the transport starts
- THEN it MAY open only the configured endpoints and socket.

#### Scenario: Drift or invalid limits
r[kiln.iroh.workflow.profile.scenario.invalid]
- GIVEN cohort drift, an empty peer policy, unsafe socket path or invalid bounds
- WHEN admission runs
- THEN it MUST reject before opening an execution connection.
