# OpenTelemetry Projection Adapter Delta Specification

## ADDED Requirements

### Requirement: Versioned OpenTelemetry profile
r[kiln.otel.profile] Kiln MUST define typed OpenTelemetry profiles with exact schema, semantic, signal, resource, cardinality, sampling, redaction, transport, and bound facts.

#### Scenario: Profile matches
r[kiln.otel.profile.scenario.match]
- GIVEN a complete profile for the pinned OpenTelemetry cohort
- WHEN projection admission runs
- THEN Kiln MAY make the declared signal mappings available.

#### Scenario: Semantic convention drifts
r[kiln.otel.profile.scenario.drift]
- GIVEN a profile with an unsupported semantic-convention version or stability state
- WHEN projection admission runs
- THEN Kiln MUST reject it before record construction.

### Requirement: Deterministic telemetry projection
r[kiln.otel.projection] Equal admitted Kiln facts and equal profile facts MUST produce equal telemetry records before shell-owned time and transport facts.

#### Scenario: Attempt span projects
r[kiln.otel.projection.scenario.span]
- GIVEN one admitted attempt and its provider observations
- WHEN pure span projection runs
- THEN it MUST preserve the declared Kiln and provider identity links.

#### Scenario: Telemetry identity replaces Kiln identity
r[kiln.otel.projection.scenario.identity]
- GIVEN a projection that uses a trace or span identity as the attempt identity
- WHEN projection admission runs
- THEN Kiln MUST reject the identity substitution.

### Requirement: Bounded telemetry safety policy
r[kiln.otel.safety] Telemetry projection MUST enforce attribute, cardinality, volume, sampling, redaction, batch, queue, and drop policy before transport.

#### Scenario: Attribute contains a secret
r[kiln.otel.safety.scenario.secret]
- GIVEN a field classified as secret or undeclared raw provider data
- WHEN redaction admission runs
- THEN Kiln MUST redact or reject it according to policy before encoding.

#### Scenario: Metric key is unbounded
r[kiln.otel.safety.scenario.cardinality]
- GIVEN a proposed metric dimension outside the allowed stable key set
- WHEN cardinality admission runs
- THEN Kiln MUST reject it.

### Requirement: Non-authoritative telemetry export
r[kiln.otel.export] Telemetry delivery, loss, sampling, duplication, delay, and backend results MUST NOT change canonical CI state.

#### Scenario: Backend loses a batch
r[kiln.otel.export.scenario.loss]
- GIVEN a committed Kiln history and a lost telemetry batch
- WHEN status rebuild runs
- THEN canonical Kiln state MUST remain unchanged and rebuildable.

#### Scenario: Backend reports success
r[kiln.otel.export.scenario.success]
- GIVEN an accepted telemetry batch
- WHEN export observation admission runs
- THEN Kiln MUST NOT infer attempt or gate success.

### Requirement: OpenTelemetry adapter conformance
r[kiln.otel.conformance] Maintained conformance MUST cover every enabled signal with mapping, drift, cardinality, redaction, loss, and authority fixtures.

#### Scenario: Secret fixture is absent
r[kiln.otel.conformance.scenario.secret]
- GIVEN an adapter candidate without secret-redaction rejection evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: OpenTelemetry adapter non-claims
r[kiln.otel.nonclaims] Passing checks MUST NOT claim complete telemetry, backend durability, provider correctness, CI correctness, or release readiness.

#### Scenario: Export passes
r[kiln.otel.nonclaims.scenario.scope]
- GIVEN a passing export fixture
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the checked projection, batch, profile, transport, and backend observations.
