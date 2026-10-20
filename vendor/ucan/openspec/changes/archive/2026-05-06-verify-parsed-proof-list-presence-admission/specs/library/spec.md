## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST parse compact UCAN tokens only when compact framing, decoded JSON object envelopes, parsed header fields, and parsed required-claims presence, including proof-list presence, are admitted before structural claim validation continues.

#### Scenario: Parsed proof-list presence uses verified bridge [r[library.compact-token.scenario.parsed-proof-list-presence-admission]]

- GIVEN serde-decoded UCAN payload claims with optional proof-list field `prf`
- WHEN the shell validates the decoded token
- THEN it MUST route proof-list presence together with the other required parsed claim presence facts through a vendored verified parsed-claims-presence bridge before constructing core claim input
- AND a missing proof-list field MUST reject instead of authorizing the token
- AND a present empty proof-list field MAY continue into structural validation

#### Scenario: Parsed proof-list presence keeps proof semantics outside the verified claim [r[library.compact-token.scenario.parsed-proof-list-presence-boundary-limits]]

- GIVEN parsed proof-list presence facts admitted by the verified predicate
- WHEN the shell continues into proof-reference decoding, proof traversal, backend resolution, revocation, temporal, or signature validation
- THEN the library MUST continue to document proof-reference string decoding, base64url decoding, BLAKE3 proof-reference derivation, proof-store/resolver/revocation truth and freshness, signing-input construction, cryptography, and token provenance as outside the parsed-presence admission claim
