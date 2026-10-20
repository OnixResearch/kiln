## ADDED Requirements

### Requirement: Verified compact token codec structure [r[library.verified-compact-token-codec-structure]]
The library MUST route compact-token segment structure, signing-input reconstruction, compact reconstruction, and decoded JSON object-envelope index discovery through vendored verified-logic predicates before accepting structural compact-token parsing.

#### Scenario: Compact token segment structure is exact [r[library.verified-compact-token-codec-structure.scenario.segment-structure]]
- GIVEN a compact token string split by the shell
- WHEN the shell decides whether the compact framing is structurally admissible
- THEN it MUST verify that the token has exactly header, payload, and signature segments
- AND each accepted segment MUST be separator-free within the verified compact-framing predicate

#### Scenario: Signing input provenance is verified [r[library.verified-compact-token-codec-structure.scenario.signing-input]]
- GIVEN encoded header, payload, and signature segments
- WHEN the shell constructs the signing input and compact reconstruction
- THEN it MUST route the facts that signing input equals `header.payload` and compact token equals `header.payload.signature` through verified logic
- AND signature verification MUST use only the admitted signing-input bytes

#### Scenario: JSON object envelope index discovery is verified [r[library.verified-compact-token-codec-structure.scenario.json-envelope-indexes]]
- GIVEN decoded header or payload bytes after runtime base64url decoding
- WHEN the shell discovers the first and last significant JSON bytes for object-envelope admission
- THEN the index discovery and object-envelope admission SHOULD move into a verified helper rather than relying on shell-supplied indexes
- AND malformed or non-object decoded payloads MUST fail closed before serde-derived claims are accepted

#### Scenario: Codec structure keeps codecs and parsers outside the claim [r[library.verified-compact-token-codec-structure.scenario.boundary-limits]]
- GIVEN compact-token structural facts admitted by verified logic
- WHEN the shell relies on base64url decoding, JSON parsing, duplicate-key behavior, canonicalization, UTF-8/string handling, or Ed25519 verification
- THEN the library MUST continue to document those facts as runtime trust boundaries outside the compact-token structure claim
