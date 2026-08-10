# Valence Evidence Adapter Delta Specification

## ADDED Requirements

### Requirement: Versioned Valence evidence profile
r[kiln.valence.profile] Kiln MUST admit Valence through a typed profile with exact schema, role, span, subject, limit, transport, and non-claim facts.

#### Scenario: Profile matches
r[kiln.valence.profile.scenario.match]
- GIVEN a complete profile for the pinned Valence cohort
- WHEN evidence-provider admission runs
- THEN Kiln MAY make the declared projection capabilities available.

#### Scenario: Role registry drifts
r[kiln.valence.profile.scenario.role]
- GIVEN a profile with an unsupported role or schema version
- WHEN provider admission runs
- THEN Kiln MUST reject it before projection.

### Requirement: Exact Valence projection
r[kiln.valence.projection] Every Valence artifact MUST bind exact Kiln subjects, source facts, evidence facts, roles, spans, algorithms, and required non-claims.

#### Scenario: Attempt evidence projects
r[kiln.valence.projection.scenario.attempt]
- GIVEN one admitted attempt and its bounded provider observations
- WHEN pure projection runs
- THEN it MUST produce one deterministic Evidence IR value with exact subject linkage.

#### Scenario: Builder role is promoted
r[kiln.valence.projection.scenario.builder]
- GIVEN source facts that name Mantle as builder
- WHEN a projection names Kiln as builder
- THEN projection admission MUST reject the role promotion.

### Requirement: Non-authoritative Valence import
r[kiln.valence.import] Imported Valence artifacts MUST remain linkage observations and MUST NOT mutate canonical CI outcomes or policy.

#### Scenario: Imported artifact reports success
r[kiln.valence.import.scenario.success]
- GIVEN a valid external artifact that reports success without a matching Kiln attempt
- WHEN import admission runs
- THEN Kiln MUST retain it as an observation without creating canonical success.

#### Scenario: Subject identity is stale
r[kiln.valence.import.scenario.stale]
- GIVEN an artifact linked to an earlier candidate
- WHEN import admission runs for the current candidate
- THEN Kiln MUST reject its applicability.

### Requirement: Valence adapter conformance
r[kiln.valence.conformance] Maintained conformance MUST include pinned producer fixtures and independent exact round-trip tests.

#### Scenario: Non-claim fixture is absent
r[kiln.valence.conformance.scenario.nonclaim]
- GIVEN an adapter candidate without missing-non-claim rejection evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Valence adapter non-claims
r[kiln.valence.nonclaims] Passing checks MUST NOT claim behavioral correctness, provider correctness, evidence truth, release eligibility, or deployment success.

#### Scenario: Linkage validates
r[kiln.valence.nonclaims.scenario.scope]
- GIVEN a Valence artifact whose identity and linkage validate
- WHEN Kiln records evidence
- THEN it MUST report only the checked identity, role, span, linkage, and non-claim facts.
