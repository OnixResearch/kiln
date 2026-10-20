## ADDED Requirements

### Requirement: Verified WOT evidence helper bridges [r[library.verified-wot-evidence-helpers]]

The library MUST expose deterministic, finite, data-only Web-of-Trust evidence helper bridges only when each helper routes through an exact vendored verified-logic attestation, revocation, or trust-threshold primitive.

#### Scenario: Attestation evidence helper uses vendored semantics [r[library.verified-wot-evidence-helpers.scenario.attestation-vendored-semantics]]

- GIVEN caller-supplied public WOT attestation evidence fields and explicit validation inputs
- WHEN the caller invokes a public attestation evidence helper
- THEN the helper MUST route through the corresponding vendored `web_of_trust_attestation` primitive
- AND semantic denial or malformed public evidence MUST be returned as a structured false/error outcome without panicking

#### Scenario: Revocation evidence helper uses vendored semantics [r[library.verified-wot-evidence-helpers.scenario.revocation-vendored-semantics]]

- GIVEN caller-supplied public WOT revocation and attestation evidence fields plus explicit time inputs
- WHEN the caller invokes a public revocation evidence helper
- THEN the helper MUST route through the corresponding vendored `web_of_trust_revocation` primitive
- AND non-matching, inactive, or invalid public evidence MUST be returned as a structured false/error outcome without panicking

#### Scenario: Evidence helpers preserve shell boundaries [r[library.verified-wot-evidence-helpers.scenario.boundary-preserved]]

- GIVEN public WOT evidence bytes and no caller-supplied resolver, store, registry, signer, or clock
- WHEN the caller invokes any WOT evidence helper
- THEN the helper MUST NOT resolve identities, fetch proofs, fetch trust records, perform hidden registry freshness checks, access storage or network backends, sign, persist state, replay-protect, or read an ambient clock
