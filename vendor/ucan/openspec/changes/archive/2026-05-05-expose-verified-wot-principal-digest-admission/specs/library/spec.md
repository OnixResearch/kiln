## ADDED Requirements

### Requirement: Verified WOT principal digest admission bridge [r[library.verified-wot-principal-digest-admission]]

The library MUST expose a deterministic, data-only Web-of-Trust principal digest admission helper that validates raw caller-supplied principal digest bytes through vendored verified-logic primitives.

#### Scenario: Full-width digest is admitted [r[library.verified-wot-principal-digest-admission.scenario.full-width-admitted]]

- GIVEN raw principal digest bytes with the vendored principal digest width
- WHEN the caller checks the bytes through the public verified bridge
- THEN the helper returns true

#### Scenario: Non-full-width digest is rejected [r[library.verified-wot-principal-digest-admission.scenario.invalid-width-rejected]]

- GIVEN raw principal digest bytes that are shorter or longer than the vendored principal digest width
- WHEN the caller checks the bytes through the public verified bridge
- THEN the helper returns false

#### Scenario: Boundary excludes WOT evidence validation [r[library.verified-wot-principal-digest-admission.scenario.boundary-excludes-evidence-validation]]

- GIVEN raw digest bytes and no caller-supplied Web-of-Trust evidence
- WHEN the caller checks the bytes through the public verified bridge
- THEN the helper only validates fixed-width digest admission and MUST NOT resolve identities, fetch or validate trust records, validate attestations, check revocation freshness, authorize UCANs, access storage or network backends, sign, replay-protect, or read an ambient clock
