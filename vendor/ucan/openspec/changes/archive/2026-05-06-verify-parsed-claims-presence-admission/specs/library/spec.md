## MODIFIED Requirements

### Requirement: End-to-end token round-trip
The library MUST parse compact UCAN tokens only when compact framing, decoded JSON object envelopes, parsed header fields, and parsed required-claims presence are admitted before structural claim validation continues.

#### Scenario: Parsed required claims presence uses verified bridge [r[library.compact-token.scenario.parsed-claims-presence-admission]]
- GIVEN serde-decoded UCAN payload claims with optional issuer, audience, attenuation, expiration, and not-before fields
- WHEN the shell validates the decoded token
- THEN it MUST route the parsed field-presence facts through a vendored verified parsed-claims-presence bridge before constructing core claim input
- AND missing issuer, audience, attenuation, expiration, or not-before presence MUST reject instead of authorizing the token

#### Scenario: Parsed claims presence keeps parser semantics outside the verified claim [r[library.compact-token.scenario.parsed-claims-presence-boundary-limits]]
- GIVEN parsed claims presence facts admitted by the verified predicate
- WHEN the shell continues into core claims, capability, proof, temporal, or signature validation
- THEN the library MUST continue to document serde parsing, duplicate-key handling, field extraction correctness, string validation, capability/proof parsing, temporal ordering, signing-input construction, cryptography, and token provenance as outside the parsed-presence admission claim
