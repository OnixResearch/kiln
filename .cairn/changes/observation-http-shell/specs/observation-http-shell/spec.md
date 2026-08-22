# HTTP Observation Shell Delta Specification

## ADDED Requirements

### Requirement: Bounded HTTP observation profile
r[kiln.observe.http.profile] Kiln MUST define typed HTTP observation profiles that declare one registered origin, TLS policy, auth admission, view and SSE content contracts, exact named bounds, and non-claims.

#### Scenario: Profile matches the contract
r[kiln.observe.http.profile.scenario.match]
- GIVEN a complete HTTP observation profile for a registered origin
- WHEN HTTP observation admission runs
- THEN Kiln MAY serve the projection views under the declared bounds.

#### Scenario: Profile drifts from the contract
r[kiln.observe.http.profile.scenario.drift]
- GIVEN an HTTP profile with a missing bound, an unsupported content contract, or a non-registered origin
- WHEN HTTP observation admission runs
- THEN Kiln MUST reject it before serving.

### Requirement: Bounded HTTPS view serving
r[kiln.observe.http.serve] The HTTP shell MUST serve admitted `kiln.observe` views as canonical JSON over HTTPS, and MUST reject unknown view kinds and unknown targets without projecting them.

#### Scenario: Admitted view is served
r[kiln.observe.http.serve.scenario.view]
- GIVEN an admitted view request under the profile
- WHEN the HTTP shell serves it
- THEN it MUST return the canonical projected view with its `kiln.observe.view.v1` identity.

#### Scenario: View kind is unknown
r[kiln.observe.http.serve.scenario.unknown_kind]
- GIVEN a request whose view kind is not in the projection vocabulary
- WHEN the HTTP shell admits it
- THEN it MUST return a bounded rejection without claiming a projection.

### Requirement: Bounded live event stream
r[kiln.observe.http.live] The HTTP shell MUST expose a bounded live event stream driven by committed journal events, and MUST keep stream loss and lag non-authoritative.

#### Scenario: Committed event is delivered
r[kiln.observe.http.live.scenario.delivery]
- GIVEN an open live stream and a newly committed journal event
- WHEN the stream advances
- THEN the shell MAY deliver the bounded event fact to the subscriber.

#### Scenario: Stream loses a fact
r[kiln.observe.http.live.scenario.loss]
- GIVEN a bounded live stream that drops or delays a fact
- WHEN canonical state rebuild runs
- THEN Kiln state MUST remain unchanged and the loss MUST be reported under the declared policy.

### Requirement: HTTP observation auth
r[kiln.observe.http.auth] The HTTP shell MUST admit every request against the profile auth policy before serving, and auth failure MUST be bounded and must not touch canonical state.

#### Scenario: Authorized request is served
r[kiln.observe.http.auth.scenario.authorized]
- GIVEN a request that satisfies the profile auth policy
- WHEN the HTTP shell admits it
- THEN the requested view MAY be served.

#### Scenario: Unauthorized request is rejected
r[kiln.observe.http.auth.scenario.unauthorized]
- GIVEN a request that fails the profile auth policy
- WHEN the HTTP shell admits it
- THEN Kiln MUST return a bounded rejection before projection.

### Requirement: Maintained HTTP observation conformance
r[kiln.observe.http.conformance] Maintained conformance MUST cover serving, rejection, auth, TLS, live delivery and loss, malformed requests, and timeouts with positive and negative fixtures.

#### Scenario: Negative fixture is missing
r[kiln.observe.http.conformance.scenario.fixture_missing]
- GIVEN an HTTP observation candidate whose fixture set lacks a required negative case
- WHEN conformance runs
- THEN the candidate MUST fail.

### Requirement: HTTP observation non-claims
r[kiln.observe.http.nonclaims] Passing HTTP observation checks MUST NOT claim transport acceptance, live completeness, control execution, provider correctness, CI success, or release readiness.

#### Scenario: A view is served
r[kiln.observe.http.nonclaims.scenario.scope]
- GIVEN a served view under a passing fixture
- WHEN Kiln records evidence
- THEN the evidence MUST remain scoped to the checked profile, request, projection, bounds, and stream observation.
