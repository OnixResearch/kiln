# Operational Radicle Shell Delta Specification

## ADDED Requirements

### Requirement: Concrete Radicle shell profile
r[kiln.radicle.shell_profile] Kiln MUST provide separate versioned Radicle profiles for source and trigger capabilities.

#### Scenario: Deployment enables source only
r[kiln.radicle.shell_profile.scenario.narrow]
- GIVEN a valid Radicle source profile without trigger capability
- WHEN provider admission runs
- THEN Kiln MUST admit source operations and reject trigger intake.

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

### Requirement: Bounded Radicle broker trigger intake
r[kiln.radicle.shell_trigger] Trigger intake MUST preserve the exact broker event, repository, revision, and durable replay facts without inferring actor-signature evidence absent from the broker schema.

#### Scenario: Valid event arrives once
r[kiln.radicle.shell_trigger.scenario.valid]
- GIVEN one valid local broker event for an admitted repository and revision
- WHEN trigger intake runs
- THEN Kiln MAY create one bounded trigger observation without claiming an authenticated actor.

#### Scenario: Event is replayed
r[kiln.radicle.shell_trigger.scenario.replay]
- GIVEN an admitted native event identity that arrives again
- WHEN trigger intake runs
- THEN Kiln MUST reject or classify the duplicate without creating another command.

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
