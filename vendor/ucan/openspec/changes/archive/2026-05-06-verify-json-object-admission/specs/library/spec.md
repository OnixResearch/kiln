## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]
The library MUST route compact-token split, framing, proof-reference digest, base64url segment, and decoded JSON object structural admission through vendored verified-logic predicates before continuing into trusted runtime codec, JSON parser, or cryptographic boundaries.

#### Scenario: Decoded JSON object admission rejects non-object header and payload bytes [r[library.verified-compact-token-codec-boundary.scenario.json-object-admission]]
- GIVEN decoded compact header or payload bytes whose first and last non-JSON-whitespace bytes do not delimit a JSON object
- WHEN the shell prepares to deserialize that decoded part
- THEN the shell MUST reject it through the verified bridge before relying on serde-derived header or claim semantics

#### Scenario: JSON object admission preserves parser trust boundary [r[library.verified-compact-token-codec-boundary.scenario.json-object-boundary-limits]]
- GIVEN decoded compact header or payload bytes admitted by the verified JSON object structural predicate
- WHEN the shell deserializes JSON, validates fields, constructs signing input, hashes proof references, or verifies signatures
- THEN the library MUST continue to document those runtime parser, codec, and cryptographic operations as outside the verified structural admission claim
