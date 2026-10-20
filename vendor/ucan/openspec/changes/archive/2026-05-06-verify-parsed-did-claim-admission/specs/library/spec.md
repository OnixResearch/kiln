## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST verify parsed issuer and audience DID claim shape facts before decoded-token structural validation accepts those claims.

#### Scenario: Parsed DID claim admission uses verified bridge [r[library.compact-token.scenario.parsed-did-claim-admission]]

- GIVEN a compact-token payload whose decoded `iss` and `aud` fields have been extracted by the shell
- WHEN decoded-token validation admits the issuer and audience claims before core claim construction
- THEN the library MUST route issuer/audience presence, non-empty, and trimmed facts through a vendored verified DID-claim admission bridge
- AND empty or untrimmed issuer/audience values MUST fail closed before the token is structurally accepted

#### Scenario: Parsed DID claim admission keeps identity semantics outside the verified claim [r[library.compact-token.scenario.parsed-did-claim-boundary-limits]]

- GIVEN parsed DID claim facts admitted by the verified predicate
- WHEN the shell later resolves keys or relies on issuer/audience identity semantics
- THEN the library MUST continue to document JSON parser correctness, UTF-8/string extraction, duplicate-key behavior, canonicalization, DID-method semantics, key-resolution truth, signature correctness, backend behavior, and token provenance as outside the parsed DID admission claim
