## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST admit parsed compact-token proof-reference lists only when every present encoded proof-reference entry satisfies verified finite list-entry shape rules before decoding continues.

#### Scenario: Parsed proof-reference list entries use verified bridge [r[library.compact-token.scenario.parsed-proof-reference-list-shape-admission]]

- GIVEN serde-decoded UCAN payload claims with a present `prf` list of encoded proof-reference entries
- WHEN the shell validates decoded proof references
- THEN it MUST route every parsed encoded proof-reference entry through a vendored verified proof-reference-list-shape bridge before base64url decoding or core proof input construction
- AND any malformed encoded entry MUST reject instead of authorizing the token

#### Scenario: Parsed proof-reference list shape keeps proof semantics outside the verified claim [r[library.compact-token.scenario.parsed-proof-reference-list-shape-boundary-limits]]

- GIVEN parsed proof-reference entry shape facts admitted by the verified predicate
- WHEN the shell continues into decoding, proof traversal, backend resolution, revocation, or signature validation
- THEN the library MUST continue to document base64url decoder correctness, decoded bytes, BLAKE3 proof-reference derivation, proof material truth, backend freshness/provenance, signing-input construction, cryptography, and token provenance as outside the parsed proof-reference-list-shape admission claim
