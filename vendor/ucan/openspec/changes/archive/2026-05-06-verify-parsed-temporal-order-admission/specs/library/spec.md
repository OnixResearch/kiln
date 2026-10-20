## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST parse compact UCAN tokens only when compact framing, decoded JSON object envelopes, parsed header fields, parsed required-claims presence, and parsed temporal ordering are admitted before structural claim validation continues.

#### Scenario: Parsed temporal order uses verified bridge [r[library.compact-token.scenario.parsed-temporal-order-admission]]

- GIVEN serde-decoded UCAN payload claims with present numeric `nbf` and `exp` values
- WHEN the shell validates the decoded token
- THEN it MUST route the parsed timestamp order fact through a vendored verified temporal-order bridge before constructing core claim input
- AND `nbf >= exp` MUST reject instead of authorizing the token

#### Scenario: Parsed temporal order keeps runtime time semantics outside the verified claim [r[library.compact-token.scenario.parsed-temporal-order-boundary-limits]]

- GIVEN parsed timestamp order facts admitted by the verified predicate
- WHEN the shell continues into current-time validation, proof traversal, backend resolution, revocation, or signature validation
- THEN the library MUST continue to document JSON parsing, numeric deserialization, duplicate-key handling, wall-clock freshness, clock source correctness, expiration-at-now policy, proof/backend truth, signing-input construction, cryptography, and token provenance as outside the parsed-temporal-order admission claim
