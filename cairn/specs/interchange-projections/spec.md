# Interchange Projections Specification

## Purpose

Defines the `interchange-projections` capability.

## Requirements

### Requirement: Non-authoritative interchange projections
r[kiln.interchange.authority] SLSA, in-toto, CDEvents, CloudEvents, OpenTelemetry, JSON, and provider-native interchange artifacts MUST remain versioned projections or observations and MUST NOT replace canonical Kiln events, identities, state, or policy.

#### Scenario: Imported event reports success
r[kiln.interchange.authority.scenario.import_success]
- GIVEN a valid CDEvents success message without a matching admitted Kiln attempt
- WHEN import admission runs
- THEN it MUST remain an external observation and MUST NOT create canonical success state.

### Requirement: Exact provenance mapping
r[kiln.interchange.provenance] A SLSA or in-toto provenance projection MUST bind exact Kiln pipeline revision, candidate or source snapshot, external and internal parameters, resolved dependencies, producing attempt, provider trust facts, products, metrics, and byproducts when those facts are available.

#### Scenario: Product subject names another attempt
r[kiln.interchange.provenance.scenario.attempt_mismatch]
- GIVEN a provenance projection whose subject is linked to a different producing attempt
- WHEN projection validation runs
- THEN validation MUST fail without changing canonical product state.

### Requirement: Builder and signer separation
r[kiln.interchange.builder] Interchange evidence MUST preserve builder, signer, provider, worker, and Kiln control-plane identities as distinct typed facts and MUST NOT name Kiln as the builder unless Kiln performed the claimed build role.

#### Scenario: Mantle produced the build
r[kiln.interchange.builder.scenario.mantle]
- GIVEN an admitted Mantle build observation exported through Kiln
- WHEN SLSA provenance is projected
- THEN the builder identity MUST describe the admitted Mantle trust boundary and MUST NOT imply that Kiln executed the build.

### Requirement: Native algorithm preservation
r[kiln.interchange.algorithms] Interchange projections MUST preserve every protocol-required digest, signature, encoding, and identifier while keeping Kiln BLAKE3 identities in separate supported fields.

#### Scenario: Protocol requires SHA-256
r[kiln.interchange.algorithms.scenario.sha256]
- GIVEN a protocol artifact whose required subject digest uses SHA-256
- WHEN Kiln exports the artifact
- THEN the required SHA-256 value MUST remain present and MUST NOT be replaced by BLAKE3.

### Requirement: Versioned CDEvents mapping
r[kiln.interchange.cdevents] CDEvents import and export MUST pin supported specification, event, subject, and CloudEvents binding versions and MUST map each supported event through a bounded pure conversion.

#### Scenario: Unknown CDEvents type arrives
r[kiln.interchange.cdevents.scenario.unknown]
- GIVEN a valid CloudEvents envelope with an unsupported CDEvents type
- WHEN import admission runs
- THEN it MUST fail as unsupported without creating a Kiln command.

### Requirement: Correlation and duplicate policy
r[kiln.interchange.correlation] Imported and exported interchange events MUST preserve native event identity, source, subject, time observation, correlation, causation, and duplicate-policy facts without using them as primary Kiln identities.

#### Scenario: Provider replays one event
r[kiln.interchange.correlation.scenario.replay]
- GIVEN two deliveries with the same admitted native event identity
- WHEN trigger intake applies the retained duplicate policy
- THEN it MUST produce the configured duplicate observation and MUST NOT create an unlinked second run.

### Requirement: OpenTelemetry projection boundary
r[kiln.interchange.telemetry] OpenTelemetry CI/CD spans, metrics, and logs MUST be rebuildable bounded projections with pinned semantic-convention status and MUST NOT become lifecycle or release authority.

#### Scenario: Trace backend loses data
r[kiln.interchange.telemetry.scenario.loss]
- GIVEN an intact Kiln journal and missing telemetry data
- WHEN telemetry rebuild runs under retained-event policy
- THEN it MUST reproduce the supported projection without provider dispatch or canonical state mutation.

### Requirement: Explicit unknown-field policy
r[kiln.interchange.evolution] Every interchange profile MUST define unknown version, unknown field, missing field, extension, downgrade, and lossy-mapping behavior before import or export.

#### Scenario: Required provenance field is missing
r[kiln.interchange.evolution.scenario.missing]
- GIVEN a provenance input without a field required by the selected profile
- WHEN import validation runs
- THEN it MUST fail without silently selecting an older or weaker profile.

### Requirement: Interchange fixtures
r[kiln.interchange.fixtures] Maintained tests MUST include valid provenance, CDEvents, and OpenTelemetry projections plus invalid versions, fields, identities, algorithms, correlations, trust roles, lossy mappings, duplicates, and authority promotion.

#### Scenario: Builder-confusion fixture is absent
r[kiln.interchange.fixtures.scenario.missing]
- GIVEN a release candidate without a current builder-confusion rejection fixture
- WHEN interchange readiness runs
- THEN the candidate MUST fail the interchange fixture gate.

### Requirement: Interchange non-claims
r[kiln.interchange.nonclaims] Passing interchange checks MUST NOT claim a SLSA level, signer trust, builder correctness, provenance completeness, transport delivery, telemetry completeness, provider correctness, or release eligibility unless a separate admitted authority proves that claim.

#### Scenario: Provenance schema is valid
r[kiln.interchange.nonclaims.scenario.schema]
- GIVEN a schema-valid provenance projection
- WHEN Kiln reports validation
- THEN the report MUST claim only schema and checked-linkage conformance under the named profile.
