## ADDED Requirements

### Requirement: Verified WOT principal derivation bridge [r[library.verified-wot-principal-derivation]]

The library MUST expose a deterministic, data-only Web-of-Trust principal digest helper that derives fixed-width principal digest bytes from caller-supplied public key bytes through vendored verified-logic primitives.

#### Scenario: Deterministic fixed-width digest [r[library.verified-wot-principal-derivation.scenario.deterministic-fixed-width]]

- GIVEN public key bytes supplied by an advanced caller
- WHEN the caller derives a Web-of-Trust principal digest through the public verified bridge
- THEN the returned digest is deterministic for the same input and has the vendored principal digest width

#### Scenario: Boundary excludes trust and resolution [r[library.verified-wot-principal-derivation.scenario.boundary-excludes-trust-resolution]]

- GIVEN public key bytes and no caller-supplied Web-of-Trust evidence
- WHEN the caller derives a Web-of-Trust principal digest through the public verified bridge
- THEN the helper only returns deterministic public principal bytes and MUST NOT resolve identities, fetch trust records, validate attestations, check revocations, authorize UCANs, access storage or network backends, or read an ambient clock
