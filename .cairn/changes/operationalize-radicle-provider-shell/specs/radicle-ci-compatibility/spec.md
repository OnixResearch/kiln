# Operational Radicle Shell Delta Specification

## ADDED Requirements

### Requirement: Concrete Radicle shell profile
r[kiln.radicle.shell_profile] Kiln MUST provide separate versioned Radicle profiles for source, trigger, status, and collaboration capabilities.

#### Scenario: Deployment enables status only
r[kiln.radicle.shell_profile.scenario.narrow]
- GIVEN a valid Radicle status profile without collaboration capability
- WHEN provider admission runs
- THEN Kiln MUST admit status operations and reject collaboration mutation.

#### Scenario: Production selects the fixture service
r[kiln.radicle.shell_profile.scenario.fixture]
- GIVEN a production profile that selects `LocalFixtureService`
- WHEN adapter readiness runs
- THEN Kiln MUST reject the profile before trigger acceptance.

### Requirement: Exact Radicle source acquisition
r[kiln.radicle.shell_source] A Radicle source result MUST bind the RID, signed reference, exact revision, provider observation, and canonical tree identity.

#### Scenario: Reference remains stable
r[kiln.radicle.shell_source.scenario.stable]
- GIVEN an admitted repository whose selected reference remains stable
- WHEN source resolution and acquisition run
- THEN Kiln MUST return one exact revision and its canonical tree identity.

#### Scenario: Reference moves during acquisition
r[kiln.radicle.shell_source.scenario.moved]
- GIVEN a selected reference that moves during acquisition
- WHEN consistency admission runs
- THEN Kiln MUST reject the mixed source observation.

### Requirement: Authenticated Radicle trigger intake
r[kiln.radicle.shell_trigger] Trigger intake MUST preserve native event, actor, repository, revision, signature, and replay facts.

#### Scenario: Valid event arrives once
r[kiln.radicle.shell_trigger.scenario.valid]
- GIVEN one valid signed event for an admitted repository and revision
- WHEN trigger intake runs
- THEN Kiln MAY create one bounded trigger observation.

#### Scenario: Event is replayed
r[kiln.radicle.shell_trigger.scenario.replay]
- GIVEN an admitted native event identity that arrives again
- WHEN trigger intake runs
- THEN Kiln MUST reject or classify the duplicate without creating another command.

### Requirement: Rebuildable Radicle status
r[kiln.radicle.shell_status] Radicle status publication MUST remain an idempotent rebuildable projection without canonical CI state authority.

#### Scenario: Status write succeeds
r[kiln.radicle.shell_status.scenario.success]
- GIVEN a status effect bound to one exact attempt and idempotency key
- WHEN the provider records the status
- THEN Kiln MUST retain the native reference as a provider observation.

#### Scenario: Connection fails after acceptance
r[kiln.radicle.shell_status.scenario.unknown]
- GIVEN a status request accepted before the connection fails
- WHEN outcome classification runs
- THEN Kiln MUST record an unknown outcome and require reconciliation.

### Requirement: Explicit Radicle mutation authority
r[kiln.radicle.shell_mutation] Collaboration mutation MUST use a fresh expected-head preflight and an explicitly supplied signer capability.

#### Scenario: Expected heads match
r[kiln.radicle.shell_mutation.scenario.authorized]
- GIVEN an authorized plan whose expected heads still match
- WHEN the shell reloads and dispatches the mutation
- THEN it MAY sign and write the exact admitted operation.

#### Scenario: Signer is ambient
r[kiln.radicle.shell_mutation.scenario.ambient]
- GIVEN a mutation plan without an explicit signer capability
- WHEN mutation preflight runs
- THEN Kiln MUST reject the effect before signing or writing.

### Requirement: Radicle shell conformance and non-claims
r[kiln.radicle.shell_conformance] Maintained conformance MUST cover every enabled capability with positive and negative local fixtures.

#### Scenario: Adapter has only protocol fixtures
r[kiln.radicle.shell_conformance.scenario.fixture_only]
- GIVEN an adapter candidate without a concrete runtime shell
- WHEN adapter readiness runs
- THEN the implemented adapter gate MUST fail.

### Requirement: Radicle shell claim boundary
r[kiln.radicle.shell_nonclaims] Passing shell checks MUST NOT claim peer convergence, actor trust, remote availability, source correctness, or mutation success.

#### Scenario: Local observation passes
r[kiln.radicle.shell_nonclaims.scenario.local]
- GIVEN a passing local repository and protocol fixture
- WHEN Kiln records evidence
- THEN the evidence MUST remain limited to the checked local inputs, identities, policy, and observations.
