# GitHub Provider Adapters Delta Specification

## ADDED Requirements

### Requirement: Narrow GitHub profiles
r[kiln.github.profile] Kiln MUST define separate typed GitHub profiles for trigger, status, collaboration read, and collaboration mutation capabilities.

#### Scenario: Trigger profile is selected
r[kiln.github.profile.scenario.trigger]
- GIVEN a valid trigger profile without API mutation scopes
- WHEN provider admission runs
- THEN Kiln MUST admit trigger intake and reject status or merge mutation.

#### Scenario: Mirror gains canonical authority
r[kiln.github.profile.scenario.authority]
- GIVEN a GitHub mirror profile marked as canonical Radicle authority
- WHEN profile admission runs
- THEN Kiln MUST reject the authority promotion.

### Requirement: Authenticated GitHub trigger intake
r[kiln.github.trigger] GitHub trigger intake MUST validate exact body bytes, HMAC-SHA256, delivery identity, repository scope, event schema, and replay state.

#### Scenario: Valid delivery arrives
r[kiln.github.trigger.scenario.valid]
- GIVEN one valid signed supported delivery for an admitted repository
- WHEN trigger intake runs
- THEN Kiln MAY create one bounded trigger observation.

#### Scenario: Delivery signature is invalid
r[kiln.github.trigger.scenario.signature]
- GIVEN a delivery with an invalid required HMAC-SHA256 signature
- WHEN trigger intake runs
- THEN Kiln MUST reject it before command admission.

### Requirement: Rebuildable GitHub status
r[kiln.github.status] GitHub status and check-run writes MUST bind exact attempts and remain rebuildable provider projections.

#### Scenario: Status write is linked
r[kiln.github.status.scenario.success]
- GIVEN a status effect bound to one attempt, commit, context, and idempotency key
- WHEN GitHub records it
- THEN Kiln MUST retain the native status reference as an observation.

#### Scenario: Write outcome is ambiguous
r[kiln.github.status.scenario.unknown]
- GIVEN a write accepted before transport disconnects
- WHEN classification runs
- THEN Kiln MUST record unknown and reconcile through exact native lookup.

### Requirement: Bounded GitHub collaboration observations
r[kiln.github.collaboration] Collaboration reads MUST preserve pull-request, head, base, commit, review, check, actor, freshness, and API-source facts.

#### Scenario: Pull request is observed
r[kiln.github.collaboration.scenario.read]
- GIVEN an admitted pull request and bounded API response
- WHEN collaboration mapping runs
- THEN Kiln MUST preserve its native facts without replacing the candidate identity.

#### Scenario: API response is stale
r[kiln.github.collaboration.scenario.stale]
- GIVEN a response that predates the required freshness policy
- WHEN collaboration admission runs
- THEN Kiln MUST reject its use for current merge preflight.

### Requirement: Fresh GitHub merge preflight
r[kiln.github.merge] GitHub merge mutation MUST require explicit authority and a fresh match for target, candidate commits, approvals, checks, capability, and policy.

#### Scenario: Preflight matches
r[kiln.github.merge.scenario.valid]
- GIVEN an authorized merge effect whose fresh facts match
- WHEN mutation dispatch runs
- THEN the adapter MAY request the exact GitHub merge operation.

#### Scenario: Target moved
r[kiln.github.merge.scenario.moved]
- GIVEN a successful gate result followed by a moved target
- WHEN merge preflight runs
- THEN Kiln MUST reject the stale merge effect.

### Requirement: GitHub adapter conformance
r[kiln.github.conformance] Maintained conformance MUST cover signatures, replay, status ambiguity, rate limits, stale reads, and merge preflight.

#### Scenario: Replay fixture is absent
r[kiln.github.conformance.scenario.replay]
- GIVEN an adapter candidate without replay rejection evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: GitHub adapter non-claims
r[kiln.github.nonclaims] Passing checks MUST NOT claim account trust, service availability, mirror freshness, source correctness, or merge success.

#### Scenario: API operation passes
r[kiln.github.nonclaims.scenario.scope]
- GIVEN a passing GitHub observation
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the exact request, response, profile, identity, freshness, and policy facts.
