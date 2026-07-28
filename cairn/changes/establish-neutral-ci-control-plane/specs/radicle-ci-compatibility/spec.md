# Radicle CI Compatibility Specification

## ADDED Requirements

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
