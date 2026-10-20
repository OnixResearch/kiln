## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]

The library MUST expose and use a verified compact-token framing bridge for already-encoded compact token parts while continuing to distinguish trusted runtime codec, hash, and cryptographic boundaries from verified structural admission. The bridge MUST NOT claim base64url, serde JSON, BLAKE3, Ed25519, resolver, proof-store, revocation-backend, caveat-policy, filesystem, network, signing, persistence, or ambient-clock behavior is formally verified.

#### Scenario: Structural compact-token admission is explicit [r[library.verified-compact-token-codec-boundary.scenario.structural-admission]]

- GIVEN malformed compact token strings with missing parts, extra parts, empty components, invalid encodings, or invalid JSON claims
- WHEN the shell parses or verifies those strings
- THEN it MUST return structured errors without panicking
- AND the verified compact framing helper MUST document that it proves only exact byte framing over already-encoded parts

#### Scenario: Signing input stability remains deterministic [r[library.verified-compact-token-codec-boundary.scenario.signing-input-stability]]

- GIVEN validated public header and claims data or already-split encoded compact parts
- WHEN the shell constructs signing input or proof-reference bytes
- THEN signing input construction MUST be deterministic for the same encoded header and payload bytes
- AND tests or verified helpers MUST avoid printing compact UCAN token values, bearer values, signing secrets, private keys, or credential-like logs

#### Scenario: Runtime trust boundaries are not hidden [r[library.verified-compact-token-codec-boundary.scenario.trust-boundaries]]

- GIVEN compact-token verification that uses runtime codec, hash, and cryptographic libraries
- WHEN documentation or guard scripts describe verification coverage
- THEN they MUST state that codec/hash/crypto internals remain trusted boundaries unless replaced by exact verified primitives
- AND the compact framing bridge MUST remain data-only and side-effect free

#### Scenario: Compact envelope bridge rejects mismatched bytes [r[library.verified-compact-token-codec-boundary.scenario.compact-envelope-bridge]]

- GIVEN already-split encoded header, payload, and signature parts plus caller-supplied compact and signing-input bytes
- WHEN either the compact bytes differ from `header.payload.signature` or the signing input bytes differ from `header.payload`
- THEN the verified bridge MUST reject the envelope
- AND parse-time integration MUST fail closed with a structured compact-token error rather than continuing to decode or verify the token
