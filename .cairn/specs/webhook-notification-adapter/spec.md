# Webhook Notification Adapter Specification

## Purpose

Defines the `webhook-notification-adapter` capability.

## Requirements

### Requirement: Registered webhook profiles
r[kiln.webhook.profile] Kiln MUST admit webhook delivery only through typed registered endpoint profiles with explicit network and authority bounds.

#### Scenario: Registered endpoint is selected
r[kiln.webhook.profile.scenario.valid]
- GIVEN a notification request that selects one admitted endpoint profile
- WHEN provider admission runs
- THEN Kiln MAY make the declared delivery capability available.

#### Scenario: Request supplies a URL
r[kiln.webhook.profile.scenario.dynamic]
- GIVEN a notification request that supplies or changes its destination URL
- WHEN destination admission runs
- THEN Kiln MUST reject it before DNS or network effects.

### Requirement: Exact webhook request linkage
r[kiln.webhook.request] Every webhook delivery MUST bind the run, outcome, notification policy, endpoint profile, body identity, and idempotency key.

#### Scenario: Body projects deterministically
r[kiln.webhook.request.scenario.valid]
- GIVEN equal admitted notification facts and equal profile facts
- WHEN pure body projection runs
- THEN it MUST produce equal bounded request bodies.

#### Scenario: Body exceeds its bound
r[kiln.webhook.request.scenario.bound]
- GIVEN a projected body that exceeds the endpoint profile bound
- WHEN request admission runs
- THEN Kiln MUST reject it before secret access or transport.

### Requirement: Bounded webhook outcome handling
r[kiln.webhook.outcome] Delivery outcomes MUST distinguish definite rejection, rate limit, timeout, provider error, and ambiguous acceptance.

#### Scenario: Receiver rejects before acceptance
r[kiln.webhook.outcome.scenario.reject]
- GIVEN a definite retryable rejection and remaining retry budget
- WHEN retry admission runs
- THEN Kiln MAY plan another delivery under the configured policy.

#### Scenario: Connection fails after possible acceptance
r[kiln.webhook.outcome.scenario.unknown]
- GIVEN a delivery that can have reached the receiver before disconnect
- WHEN outcome classification runs
- THEN Kiln MUST record unknown and reject unsafe automatic retry.

### Requirement: Constrained webhook network authority
r[kiln.webhook.network] Webhook transport MUST enforce configured scheme, host, port, path, redirect, address-range, and response bounds.

#### Scenario: Redirect changes authority
r[kiln.webhook.network.scenario.redirect]
- GIVEN a receiver redirect to a destination outside the admitted profile
- WHEN redirect admission runs
- THEN Kiln MUST reject it without contacting the new destination.

#### Scenario: DNS resolves to a blocked range
r[kiln.webhook.network.scenario.address]
- GIVEN an admitted host whose resolved address violates network policy
- WHEN connection admission runs
- THEN Kiln MUST reject it before connection.

### Requirement: Webhook adapter conformance
r[kiln.webhook.conformance] Maintained conformance MUST cover destinations, redirects, signatures, secrets, retries, timeouts, ambiguity, and redaction.

#### Scenario: Ambiguous fixture is absent
r[kiln.webhook.conformance.scenario.unknown]
- GIVEN an adapter candidate without ambiguous-acceptance evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Webhook adapter non-claims
r[kiln.webhook.nonclaims] Passing checks MUST NOT claim human receipt, endpoint trust, message effect, or canonical CI outcome.

#### Scenario: Receiver accepts
r[kiln.webhook.nonclaims.scenario.scope]
- GIVEN an accepted delivery response
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the checked request, response, endpoint profile, and transport observations.
