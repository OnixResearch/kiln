# Mantle Native Report Specification

## Purpose

Defines the `mantle-native-report` capability.

## Requirements

### Requirement: Complete native report surface

r[kiln.mantle.native_report.surface] The adapter MUST admit the complete top-level `crunch-build-report-v1` surface serialized by the pinned Mantle cohort and MUST reject an unknown top-level field, a missing mandatory producer field, or a schema mismatch.

#### Scenario: Complete producer report is admitted

r[kiln.mantle.native_report.surface.complete]
- GIVEN a native report with every pinned top-level field
- WHEN completion admission runs
- THEN the report MUST be admitted
- AND every native collection count MUST be observable.

#### Scenario: Producer drift is rejected

r[kiln.mantle.native_report.surface.drift]
- GIVEN a report with an unknown top-level field, a missing mandatory field, or another schema identifier
- WHEN completion admission runs
- THEN Kiln MUST reject the report without recording canonical success.

### Requirement: Distinct logical and physical roots

r[kiln.mantle.native_report.store_binding] The profile MUST name one plan path, one logical store prefix, and one physical output directory separately, and dispatch MUST pass the roots explicitly.

#### Scenario: Report names the admitted roots

r[kiln.mantle.native_report.store_binding.match]
- GIVEN a report whose plan path, logical store prefix, physical output directory and state directory equal the profile
- WHEN completion admission runs
- THEN product paths MUST be admitted beneath the physical output root
- AND artifact logical paths MUST be admitted beneath the logical store prefix.

#### Scenario: Report names another root

r[kiln.mantle.native_report.store_binding.drift]
- GIVEN a report whose plan path, logical store prefix, physical output directory, or state directory differs from the profile
- WHEN completion admission runs
- THEN Kiln MUST reject the report before canonical completion.

### Requirement: Native count and outcome agreement

r[kiln.mantle.native_report.counts] Native counts MUST agree exactly with the outcome and failure arrays, and a successful report MUST NOT contain a failure envelope or FOD mismatch.

#### Scenario: Counts agree

r[kiln.mantle.native_report.counts.valid]
- GIVEN outcome and failure arrays with matching native totals
- WHEN completion admission runs
- THEN Kiln MUST select the outcome from the native failure facts.

#### Scenario: Summary contradicts arrays

r[kiln.mantle.native_report.counts.contradiction]
- GIVEN a count field that differs from its array length or a success report with a FOD mismatch
- WHEN completion admission runs
- THEN Kiln MUST reject the report.

### Requirement: Bounded native path facts

r[kiln.mantle.native_report.paths] Output, attestation and log paths MUST be absolute and lexically contained by the root declared for that fact.

#### Scenario: Bounded paths are admitted

r[kiln.mantle.native_report.paths.valid]
- GIVEN product paths beneath the physical output root, artifact paths beneath the state root, and logs beneath the state root
- WHEN completion admission runs
- THEN the paths MUST be bound to the exact build.

#### Scenario: Path escapes its declared root

r[kiln.mantle.native_report.paths.escape]
- GIVEN an output, artifact, or log path outside its declared root
- WHEN completion admission runs
- THEN Kiln MUST reject the report.

### Requirement: Failure facts remain inspectable

r[kiln.mantle.native_report.failures] A native failure envelope MUST retain its phase and error class within the pinned vocabulary, and its saved log MUST be readable through the exact build record when present.

#### Scenario: Failure log is bound

r[kiln.mantle.native_report.failures.log]
- GIVEN a failed native report with a saved log path beneath the admitted state root
- WHEN completion admission runs
- THEN the observation MUST be failed, MUST contain no products, and MUST expose the failure log identity.

#### Scenario: Unknown failure vocabulary

r[kiln.mantle.native_report.failures.vocabulary]
- GIVEN a failure phase or error class outside the pinned producer vocabulary
- WHEN completion admission runs
- THEN Kiln MUST reject the report.

### Requirement: Native facts are bounded observations

r[kiln.mantle.native_report.facts] Every native evidence collection, cache admission, cache transfer total and scheduler policy fact MUST be projected into deterministic bounded observation metrics, and the exact report bytes MUST remain the receipt identity.

#### Scenario: Native facts are observable

r[kiln.mantle.native_report.facts.metrics]
- GIVEN a complete admitted report
- WHEN completion admission runs
- THEN each native collection MUST have a count metric
- AND substitution bytes, fallback facts, cache admissions and scheduler policy facts MUST have metrics.

#### Scenario: Nested evidence is not promoted

r[kiln.mantle.native_report.facts.nonclaims]
- GIVEN admitted native evidence arrays
- WHEN Kiln records the observation
- THEN nested producer evidence MUST remain bound by the report receipt
- AND it MUST NOT become product, cache, CI or release authority.
