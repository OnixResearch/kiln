# Iroh Observation Shell Delta Specification

## ADDED Requirements

### Requirement: Versioned iroh observation profile
r[kiln.observe.iroh.profile] Kiln MUST define typed iroh observation profiles that declare the exact consumed n0 cohort, one ALPN protocol identity, endpoint and relay policy, peer identity and trust policy, named bounds, and non-claims.

#### Scenario: Profile matches the pinned cohort
r[kiln.observe.iroh.profile.scenario.match]
- GIVEN a complete iroh observation profile for the pinned n0 cohort
- WHEN iroh observation admission runs
- THEN Kiln MAY serve the projection views under the declared policy.

#### Scenario: Cohort drifts from the pin
r[kiln.observe.iroh.profile.scenario.drift]
- GIVEN an iroh profile with a missing bound or a cohort fact that differs from the pin
- WHEN iroh observation admission runs
- THEN Kiln MUST reject it before serving.

### Requirement: Bounded iroh view serving
r[kiln.observe.iroh.serve] The iroh shell MUST serve admitted `kiln.observe` views over the `kiln.observe.v1` ALPN protocol, and MUST keep view serving independent of transport failure.

#### Scenario: Admitted view is served to a peer
r[kiln.observe.iroh.serve.scenario.target]
- GIVEN a peer holding the ALPN protocol and an admitted view request
- WHEN the iroh shell serves it
- THEN it MUST deliver the canonical projected view with its `kiln.observe.view.v1` identity.

#### Scenario: Transport fails mid-serving
r[kiln.observe.iroh.serve.scenario.transport_failure]
- GIVEN a view request and a transport failure during serving
- WHEN the iroh shell classifies the result
- THEN it MUST report the bounded failure without claiming a served view.

### Requirement: Verified payload streaming
r[kiln.observe.iroh.verify] Log and record ranges MUST stream as content-addressed payloads, and the recipient MUST verify the declared BLAKE3 identity before claiming delivery.

#### Scenario: Verified payload arrives
r[kiln.observe.iroh.verify.scenario.verified]
- GIVEN a content-addressed payload whose bytes match its declared identity
- WHEN the recipient verifies it
- THEN the recipient MAY claim that exact payload delivered.

#### Scenario: Payload is corrupted
r[kiln.observe.iroh.verify.scenario.corrupted]
- GIVEN a content-addressed payload whose bytes do not match its declared identity
- WHEN the recipient verifies it
- THEN the payload MUST be rejected or refetched, and must not be claimed as delivered.

### Requirement: Peer identity and trust
r[kiln.observe.iroh.peer] The iroh shell MUST dial peers by public key under a declared identity and trust policy, and MUST deny unknown or revoked peers.

#### Scenario: Trusted peer is served
r[kiln.observe.iroh.peer.scenario.trusted]
- GIVEN a peer admitted under the identity and trust policy
- WHEN the peer requests a view
- THEN the shell MAY serve the view to that peer.

#### Scenario: Unknown or revoked peer is denied
r[kiln.observe.iroh.peer.scenario.denied]
- GIVEN a peer that is absent from the alias allowlist or revoked
- WHEN the peer requests a view
- THEN the shell MUST deny the request under the declared policy.

### Requirement: HTTP and iroh observation equivalence
r[kiln.observe.iroh.equiv] The HTTP and iroh observation legs MUST produce equal logical views over shared fixtures, and maintained conformance MUST reject divergent legs.

#### Scenario: Both legs serve equal views
r[kiln.observe.iroh.equiv.scenario.equal_views]
- GIVEN the same committed state and the same view request
- WHEN the HTTP and iroh legs both serve it
- THEN the logical views MUST be equal.

#### Scenario: Legs diverge
r[kiln.observe.iroh.equiv.scenario.divergence]
- GIVEN two legs that produce different logical views for equal inputs
- WHEN the equivalence rail runs
- THEN the rail MUST fail conformance.

### Requirement: Maintained iroh observation conformance
r[kiln.observe.iroh.conformance] Maintained conformance MUST cover serving, peer admission and revocation, verified streaming, corrupted-payload rejection, relay fallback, and HTTP/iroh equivalence with positive and negative fixtures.

#### Scenario: Negative fixture is missing
r[kiln.observe.iroh.conformance.scenario.fixture_missing]
- GIVEN an iroh observation candidate whose fixture set lacks a required negative case
- WHEN conformance runs
- THEN the candidate MUST fail.

### Requirement: Iroh observation non-claims
r[kiln.observe.iroh.nonclaims] Passing iroh observation checks MUST NOT claim transport acceptance, live completeness, verified delivery of unverified payloads, control execution, provider correctness, CI success, or release readiness.

#### Scenario: A peer view passes
r[kiln.observe.iroh.nonclaims.scenario.scope]
- GIVEN a passing iroh observation fixture
- WHEN Kiln records evidence
- THEN the evidence MUST remain scoped to the checked profile, protocol, peer, payload, bounds, and view identity.
