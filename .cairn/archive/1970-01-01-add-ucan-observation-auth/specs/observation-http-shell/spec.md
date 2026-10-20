# UCAN Observation Auth Delta Specification

## ADDED Requirements

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
