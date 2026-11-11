# Migration Bridge Delta Specification

## ADDED Requirements

### Requirement: Bound VM migration bridge plan

r[kiln.hosted.bridge.plan] A VM-specific migration bridge MUST bind the exact
operation, session, attempt/fence and checkpoint-manifest identity; source and
target placements; authenticated source and target endpoint public keys;
protected absolute Unix sockets; protocol generation; precopy memory mode; one
connection; and positive stream/deadline bounds. Admission MUST refuse stale or
crossed bindings, zero/equal endpoints, same-host or non-advancing placements,
unsafe sockets, postcopy or unsupported modes, multiple connections and bounds
over their named limits. Admission MUST NOT claim transfer, verification,
placement commit, resume or cleanup.

#### Scenario: Exact bridge is admitted

r[kiln.hosted.bridge.plan.scenario.admit]
- GIVEN exact operation, endpoints, sockets, checkpoint, placements and protocol generation
- WHEN bridge admission runs
- THEN one deterministic bridge identity MUST be produced

#### Scenario: Wrong peer or unsupported mode is refused

r[kiln.hosted.bridge.plan.scenario.refused]
- GIVEN a wrong endpoint/socket/protocol, a stale placement, an escaping socket or more than one connection
- WHEN bridge admission runs
- THEN it MUST fail before any transport
