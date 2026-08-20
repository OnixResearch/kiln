# Lattice Workflow Transport Client Delta Specification

## ADDED Requirements

### Requirement: Concrete bounded Lattice client

r[kiln.lattice.transport_client] Kiln MUST provide a concrete bounded Linux Unix-domain `WorkflowExchangePort` pinned to one immutable Lattice transport cohort.

#### Scenario: Local server returns a valid response
r[kiln.lattice.transport_client.scenario.valid]
- GIVEN an admitted endpoint, peer profile, and workflow request
- WHEN the concrete client exchanges one frame
- THEN it MUST preserve the exact producer request and response linkage.

#### Scenario: Endpoint is unavailable
r[kiln.lattice.transport_client.scenario.unavailable]
- GIVEN the selected socket cannot be connected
- WHEN exchange starts
- THEN Kiln MUST return an infrastructure failure without fallback or process execution.

### Requirement: Dispatch uncertainty uses lookup only

r[kiln.lattice.transport_recovery] Kiln MUST recover an unknown dispatch only through the producer's original-request lookup command and MUST NOT redispatch automatically.

#### Scenario: Acceptance is recovered
r[kiln.lattice.transport_recovery.scenario.accepted]
- GIVEN dispatch completed its write and the response was lost
- WHEN exact recovery returns the linked acceptance
- THEN Kiln MAY retain that acceptance and continue normal observation.

#### Scenario: Recovery is not found
r[kiln.lattice.transport_recovery.scenario.not-found]
- GIVEN exact recovery returns not-found
- WHEN Kiln classifies the result
- THEN it MUST retain an unknown outcome without retry authority.

### Requirement: Concrete client conformance

r[kiln.lattice.transport_conformance] Maintained tests MUST cover every operation plus malformed, oversized, stale, unavailable, not-found, and disconnect outcomes.

#### Scenario: Only protocol fixtures exist
r[kiln.lattice.transport_conformance.scenario.fixture-only]
- GIVEN no concrete local socket exchange test exists
- WHEN adapter readiness runs
- THEN the concrete transport gate MUST fail.
