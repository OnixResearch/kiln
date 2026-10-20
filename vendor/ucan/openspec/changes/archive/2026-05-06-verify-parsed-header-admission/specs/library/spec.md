## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]
The library MUST route compact-token split, framing, proof-reference digest, base64url segment, decoded JSON object structural admission, and parsed header field admission through vendored verified-logic predicates before continuing into trusted runtime codec, JSON parser, structural-validation, or cryptographic boundaries.

#### Scenario: Parsed header admission rejects unsupported algorithm or type [r[library.verified-compact-token-codec-boundary.scenario.parsed-header-admission]]
- GIVEN a compact header deserialized into parsed `alg` and `typ` fields
- WHEN either parsed field differs byte-for-byte from the supported `EdDSA` / `JWT` constants
- THEN the shell MUST reject the token through the verified parsed-header bridge before structural validation or signature verification continues

#### Scenario: Parsed header admission preserves parser and crypto trust boundaries [r[library.verified-compact-token-codec-boundary.scenario.parsed-header-boundary-limits]]
- GIVEN parsed header fields admitted by the verified predicate
- WHEN the shell relies on serde field extraction, canonical signing input, or Ed25519 verification
- THEN the library MUST continue to document those parser, canonicalization, and cryptographic operations as outside the verified parsed-header admission claim
