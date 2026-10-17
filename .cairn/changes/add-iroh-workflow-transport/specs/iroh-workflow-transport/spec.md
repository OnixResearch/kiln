# Iroh Workflow Transport Delta Specification

## ADDED Requirements

### Requirement: Versioned execution transport profile
r[kiln.iroh.workflow.profile] Kiln MUST pin Iroh 0.95.1 and ALPN `kiln.workflow.v1`, admit explicit stable identities, peer/relay policy, native Lattice contract profile, private local socket and positive finite frame/log/concurrency/deadline bounds before opening an execution connection. Core CI crates MUST remain provider-neutral.

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

### Requirement: Authenticated execution peers
r[kiln.iroh.workflow.peers] The client MUST pin the worker endpoint key; the worker MUST deny unenrolled authenticated keys before reading execution payloads or opening Lattice. The controller proxy MUST check admitted local OS user IDs and use a private socket.

#### Scenario: Enrolled peer
r[kiln.iroh.workflow.peers.scenario.allowed]
- GIVEN an authenticated enrolled peer and the admitted native contract
- WHEN it exchanges a workflow
- THEN the worker MAY forward it to its selected local Lattice socket.

#### Scenario: Unenrolled peer
r[kiln.iroh.workflow.peers.scenario.denied]
- GIVEN an authenticated but unlisted key
- WHEN it connects
- THEN it MUST NOT cause any Lattice operation.

### Requirement: Bounded native workflow transport
r[kiln.iroh.workflow.transport] The bridge MUST preserve native request/response identities for dispatch, recover-dispatch, observation, controls, logs and reconciliation; validate exact native profile, identities and response linkage; bound allocation, concurrent connections, logs and elapsed exchange time; and own shutdown of its work.

#### Scenario: Exact workflow exchange
r[kiln.iroh.workflow.transport.scenario.valid]
- GIVEN an admitted native workflow request
- WHEN it crosses both bridges
- THEN its exact response MUST remain subject to producer and Kiln admission.

#### Scenario: Malformed or stale input
r[kiln.iroh.workflow.transport.scenario.invalid]
- GIVEN an oversized or malformed frame, stale identity, mismatched profile or unlinked response
- WHEN the bridge validates it
- THEN it MUST NOT expose it as a valid successful exchange.

### Requirement: Unknown outcomes require exact recovery
r[kiln.iroh.workflow.recovery] A transport/provider failure, cancellation or deadline after possible dispatch MUST remain unknown to the existing consumer; the bridge MUST NOT synthesize success, authorize blind retry or redispatch automatically. Recovery MUST use the original exact native RecoverDispatch operation against Lattice's durable state.

#### Scenario: Reply lost after acceptance
r[kiln.iroh.workflow.recovery.scenario.unknown]
- GIVEN a request that may have reached Lattice and a lost or timed-out reply
- WHEN the local exchange ends
- THEN Kiln MUST retain unknown-after-write and recover the original request without ordinary redispatch.

### Requirement: Execution transport conformance and non-claims
r[kiln.iroh.workflow.conformance] Maintained checks MUST exercise real authenticated Iroh endpoints with positive and negative admission, framing, provider failure, timeout and recovery cases. Evidence MUST remain scoped to checked peers, profiles, inputs, binaries and local transports, and MUST NOT imply sandboxing, WAN/relay availability, provider truth, CI correctness, exactly-once execution, publication or release authority.

#### Scenario: Local remote execution proof
r[kiln.iroh.workflow.conformance.scenario.local]
- GIVEN an actual controller/proxy/worker/Lattice workflow run
- WHEN evidence is recorded
- THEN it MUST report the observed execution and bounded claim without promoting transport facts into CI authority.
