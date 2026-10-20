# Observation Http Shell Specification

## Purpose

Defines the `observation-http-shell` capability.

## Requirements

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

### Requirement: Versioned UCAN observation-auth profile
r[kiln.observe.http.ucan_profile] Kiln MUST define a typed `ucan_holder_invocation_v1` HTTP observation auth profile that declares the exact vendored UCAN cohort, root issuer, authorized holder keys, observation resource and ability, operation profile, bounded proof tokens, revoked proof references, token and nonce bounds, and non-claims.

#### Scenario: Complete UCAN profile is admitted
r[kiln.observe.http.ucan_profile.scenario.match]
- GIVEN a complete UCAN observation-auth profile for the pinned vendor cohort
- WHEN HTTP observation admission runs
- THEN Kiln MAY admit holder-invocation requests under the declared policy.

#### Scenario: UCAN cohort or bounds drift
r[kiln.observe.http.ucan_profile.scenario.drift]
- GIVEN a changed cohort fact, missing bound, unsupported operation profile, or malformed issuer
- WHEN HTTP observation admission runs
- THEN Kiln MUST reject the profile before serving.

#### Scenario: Proof token is not anchored to the profile root
r[kiln.observe.http.ucan_profile.scenario.anchor]
- GIVEN a configured proof token whose issuer chain does not reach the profile root
- WHEN HTTP observation admission runs
- THEN Kiln MUST reject the profile before serving.

### Requirement: Holder-signed invocation admission
r[kiln.observe.http.ucan_holder] The HTTP shell MUST verify the compact token, proof chain, revocation status, audience key possession, capability, operation profile, request binding, time window, and caveats before projection, and MUST reject any invocation that does not authorize the exact observation request.

#### Scenario: Authorized holder invocation is served
r[kiln.observe.http.ucan_holder.scenario.authorized]
- GIVEN a valid compact token, proof chain, and holder signature for the profile capability and exact request binding
- WHEN the HTTP shell admits the request
- THEN it MAY serve the projected view.

#### Scenario: Invocation is not anchored to the profile root
r[kiln.observe.http.ucan_holder.scenario.root]
- GIVEN a valid holder signature over a token that is self-issued or whose proof chain does not reach the profile root
- WHEN the HTTP shell admits the request
- THEN it MUST return a bounded authorization rejection without projection.

#### Scenario: Invocation does not authorize the request
r[kiln.observe.http.ucan_holder.scenario.denied]
- GIVEN a missing, malformed, expired, revoked, wrong-audience, wrong-capability, wrong-operation-profile, or wrong-request-binding invocation
- WHEN the HTTP shell admits the request
- THEN it MUST return a bounded authorization rejection without projection.

### Requirement: Bounded UCAN replay admission
r[kiln.observe.http.ucan_replay] The HTTP shell MUST admit each holder nonce at most once under the declared replay window and bound, and MUST perform replay admission only after all non-consuming token and authorization checks succeed.

#### Scenario: Replayed nonce is denied
r[kiln.observe.http.ucan_replay.scenario.replay]
- GIVEN a previously admitted audience, nonce, and request digest
- WHEN the same invocation is presented again
- THEN the shell MUST deny it without projection.

#### Scenario: Replay cache is full
r[kiln.observe.http.ucan_replay.scenario.bound]
- GIVEN a replay cache at its declared entry bound
- WHEN another valid invocation arrives
- THEN the shell MUST fail closed without projection.

### Requirement: UCAN observation-auth non-claims
r[kiln.observe.http.ucan_nonclaims] Passing UCAN observation-auth checks MUST NOT claim issuer intent, key custody, global revocation freshness, proof-store completeness, source trust, provider correctness, CI success, or release readiness.

#### Scenario: A holder invocation passes
r[kiln.observe.http.ucan_nonclaims.scenario.scope]
- GIVEN a passing holder-invocation fixture
- WHEN Kiln records evidence
- THEN the evidence MUST remain scoped to the checked profile, cohort, token, proof chain, revocation list, holder signature, operation profile, request binding, time window, replay state, and view identity.
