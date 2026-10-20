## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]
The library MUST route compact-token split, framing, proof-reference digest, and base64url segment structural admission through vendored verified-logic predicates before continuing into trusted runtime codec or cryptographic boundaries.

#### Scenario: Base64url-no-pad segment admission rejects malformed encoded parts [r[library.verified-compact-token-codec-boundary.scenario.base64url-segment-admission]]
- GIVEN an encoded compact-token segment containing a non URL-safe no-padding alphabet byte, padding byte, or impossible unpadded length class
- WHEN the shell prepares to decode that segment
- THEN the shell MUST reject it through the verified bridge before invoking or relying on decoded JSON/signature semantics

#### Scenario: Base64url-no-pad segment admission preserves codec trust boundary [r[library.verified-compact-token-codec-boundary.scenario.base64url-boundary-limits]]
- GIVEN an encoded compact-token segment admitted by the verified base64url-no-pad structural predicate
- WHEN the shell decodes, parses JSON, hashes proof references, or verifies signatures
- THEN the library MUST continue to document those runtime codec and cryptographic operations as outside the verified structural admission claim
