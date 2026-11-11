# Interchange Projections Specification

## ADDED Requirements

### Requirement: Canonical in-toto and SLSA statement emission

r[kiln.interchange.statement] Kiln MUST map an admitted, attempt-validated
`ProvenanceProjection` to canonical unsigned JSON: an in-toto Statement `v1`
whose predicate type is `https://slsa.dev/provenance/v1.1` and whose build type
is the Kiln CI-run identity. Each product MUST appear as a subject with its
BLAKE3 digest; the source or candidate and every provider-native digest MUST
appear as resolved dependencies; and builder, producing attempt, provider,
worker and Kiln control-plane identities MUST remain in distinct fields.
Products and native digests MUST be ordered before encoding so equal admitted
projections produce equal bytes.

#### Scenario: Canonical statement matches the golden document

r[kiln.interchange.statement.scenario.canonical]
- GIVEN an admitted projection and its exact producing attempt
- WHEN the statement is emitted
- THEN the bytes MUST equal the retained golden document
- AND reversing subject or digest order MUST NOT change them

#### Scenario: Refusal before serialization

r[kiln.interchange.statement.scenario.refused]
- GIVEN a projection bound to another attempt, an empty product set, or two native digests that collide on one statement key
- WHEN emission is requested
- THEN it MUST fail before producing bytes

#### Scenario: Evidence surface never claims authority

r[kiln.interchange.statement.scenario.evidence]
- GIVEN a retained provenance report containing an in-toto statement
- WHEN the run Evidence view renders it
- THEN it MUST show subjects, dependencies and distinct identities
- AND it MUST state that the unsigned statement grants no SLSA level, builder, signer, gate, merge or release authority
