# Radicle Ci Compatibility Specification

## Purpose

Defines the `radicle-ci-compatibility` capability.

## Requirements

### Requirement: Pinned Radicle broker protocol
r[kiln.radicle.protocol] The Radicle compatibility profile MUST pin the supported broker trigger and response protocol version, message shape, source revision, and interoperability limits.

#### Scenario: Broker sends an unknown protocol version
r[kiln.radicle.protocol.scenario.version]
- GIVEN a line-JSON trigger with an unsupported version
- WHEN the compatibility parser runs
- THEN it MUST reject the message without starting a Kiln run.

### Requirement: Exact trigger mapping
r[kiln.radicle.trigger] The compatibility adapter MUST map supported Radicle repository, event, actor, before, after, branch, commit, and patch facts into typed Kiln source and trigger observations without granting trust.

#### Scenario: Supported push trigger arrives
r[kiln.radicle.trigger.scenario.push]
- GIVEN a valid supported Radicle broker push message
- WHEN trigger mapping runs
- THEN it MUST produce one bounded Kiln trigger command that preserves the native facts.

### Requirement: Broker process lifecycle
r[kiln.radicle.lifecycle] The compatibility executable MUST read one trigger line, emit one triggered response after Kiln accepts the run, and emit one terminal response under named wait or polling policy.

#### Scenario: Kiln accepts the run
r[kiln.radicle.lifecycle.scenario.triggered]
- GIVEN a valid broker trigger and an admitted Kiln run
- WHEN the adapter obtains the canonical run identity
- THEN it MUST emit exactly one protocol-valid triggered response with the mapped run ID.

### Requirement: Bounded result mapping
r[kiln.radicle.result] The adapter MUST map Kiln terminal success and failure to supported broker results and MUST classify denial, cancellation, timeout, provider error, unknown outcome, output failure, and disconnect without false success.

#### Scenario: Kiln attempt remains unknown
r[kiln.radicle.result.scenario.unknown]
- GIVEN a broker-triggered run with an unresolved unknown provider outcome
- WHEN the adapter wait policy ends
- THEN the adapter MUST return or record the supported non-success classification and MUST NOT emit success.

### Requirement: Radicle status projection
r[kiln.radicle.status] An optional Radicle job COB adapter MAY project admitted Kiln run status, but the projection MUST NOT mutate canonical Kiln state or grant source, execution, or release authority.

#### Scenario: Job COB reports success
r[kiln.radicle.status.scenario.success]
- GIVEN a Radicle job COB projection that reports success
- WHEN Kiln rebuilds canonical state
- THEN it MUST use its journal and provider observations rather than treating the COB as state authority.

### Requirement: Native runner is not implicit authority
r[kiln.radicle.native_runner] Kiln MUST NOT execute `.radicle/native.yaml` shell text unless an explicit compatibility import, execution provider, trust profile, and policy admit the resulting pipeline and effect.

#### Scenario: Repository contains native shell configuration
r[kiln.radicle.native_runner.scenario.shell]
- GIVEN a repository with `.radicle/native.yaml` and no admitted compatibility profile
- WHEN a Radicle trigger enters Kiln
- THEN Kiln MUST NOT execute the shell text.

### Requirement: Radicle interoperability fixtures
r[kiln.radicle.fixtures] Maintained fixtures MUST cover valid push and patch triggers, run IDs, log links, success, and failure plus empty, malformed, unknown-version, duplicate, noisy-output, timeout, unknown-outcome, and unsafe-shell cases.

#### Scenario: Adapter writes diagnostic text to standard output
r[kiln.radicle.fixtures.scenario.stdout]
- GIVEN an adapter candidate that mixes diagnostic text with protocol responses
- WHEN protocol conformance runs
- THEN the candidate MUST fail and diagnostics MUST remain on standard error or another admitted sink.

### Requirement: Radicle compatibility non-claims
r[kiln.radicle.nonclaims] Passing Radicle compatibility checks MUST NOT claim Radicle node correctness, event completeness, repository trust, actor authority, exactly-once delivery, job COB authority, runner isolation, or CI correctness.

#### Scenario: Broker protocol fixture passes
r[kiln.radicle.nonclaims.scenario.scope]
- GIVEN a passing broker trigger and response fixture
- WHEN compatibility evidence is reported
- THEN the report MUST remain limited to the checked message mapping and process behavior.

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
