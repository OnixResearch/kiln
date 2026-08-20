# Cdevents Interchange Adapter Specification

## Purpose

Defines the `cdevents-interchange-adapter` capability.

## Requirements

### Requirement: Versioned CDEvents profile
r[kiln.cdevents.profile] Kiln MUST define a typed registry for supported CDEvents, subject, event, CloudEvents binding, field, loss, and bound facts.

#### Scenario: Event type is supported
r[kiln.cdevents.profile.scenario.supported]
- GIVEN an event type and versions present in the admitted registry
- WHEN interchange admission runs
- THEN Kiln MAY invoke its explicit bounded mapping.

#### Scenario: Event type is unknown
r[kiln.cdevents.profile.scenario.unknown]
- GIVEN a valid CloudEvents envelope with an unsupported CDEvents type
- WHEN interchange admission runs
- THEN Kiln MUST reject it without creating a command or canonical event.

### Requirement: Bounded CloudEvents envelope
r[kiln.cdevents.envelope] CloudEvents encoding and decoding MUST preserve required envelope fields and enforce version, field, text, extension, and total bounds.

#### Scenario: Envelope is valid
r[kiln.cdevents.envelope.scenario.valid]
- GIVEN a supported bounded envelope
- WHEN pure decoding runs
- THEN it MUST return exact admitted envelope facts.

#### Scenario: Required field is missing
r[kiln.cdevents.envelope.scenario.missing]
- GIVEN an envelope without a required identity or type field
- WHEN decoding runs
- THEN Kiln MUST reject it without semantic mapping.

### Requirement: Explicit CDEvents mappings
r[kiln.cdevents.mapping] Each supported event MUST use an explicit pure import and export mapping with declared loss behavior.

#### Scenario: Mapping is lossless
r[kiln.cdevents.mapping.scenario.roundtrip]
- GIVEN one supported canonical fixture
- WHEN import, export, and reimport run
- THEN every declared identity and semantic field MUST remain equal.

#### Scenario: Mapping drops a required field
r[kiln.cdevents.mapping.scenario.loss]
- GIVEN an input that requires undeclared field loss
- WHEN mapping admission runs
- THEN Kiln MUST reject it.

### Requirement: Non-authoritative CDEvents import
r[kiln.cdevents.import] Imported CDEvents values MUST remain external observations with explicit duplicate, correlation, and causation decisions.

#### Scenario: Imported event reports success
r[kiln.cdevents.import.scenario.success]
- GIVEN a valid external success event without a matching Kiln attempt
- WHEN import admission runs
- THEN Kiln MUST NOT create canonical success.

#### Scenario: Event is duplicated
r[kiln.cdevents.import.scenario.duplicate]
- GIVEN the same admitted source and native event identity twice
- WHEN duplicate admission runs
- THEN Kiln MUST emit the configured duplicate observation without a second canonical action.

### Requirement: CDEvents adapter conformance
r[kiln.cdevents.conformance] Maintained conformance MUST cover every supported type with canonical and independent positive and negative fixtures.

#### Scenario: One mapping lacks a negative fixture
r[kiln.cdevents.conformance.scenario.negative]
- GIVEN a supported mapping without malformed or drift rejection evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: CDEvents adapter non-claims
r[kiln.cdevents.nonclaims] Passing checks MUST NOT claim external event truth, delivery, provider correctness, canonical CI state, or release authority.

#### Scenario: Round trip passes
r[kiln.cdevents.nonclaims.scenario.scope]
- GIVEN a passing round-trip fixture
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the checked format, mapping, identity, and field set.
