## ADDED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]

The library MUST distinguish future structural compact-token admission verification from trusted runtime codec, hash, and cryptographic boundaries, and MUST NOT claim base64url, serde JSON, BLAKE3, or Ed25519 internals are formally verified unless exact verified primitives are added.

#### Scenario: Structural compact-token admission is explicit [r[library.verified-compact-token-codec-boundary.scenario.structural-admission]]

- GIVEN malformed compact token strings with missing parts, extra parts, empty components, invalid encodings, or invalid JSON claims
- WHEN the shell parses or verifies those strings
- THEN it MUST return structured errors without panicking
- AND any future verified helper MUST document which structural admission facts it proves

#### Scenario: Signing input stability remains deterministic [r[library.verified-compact-token-codec-boundary.scenario.signing-input-stability]]

- GIVEN validated public header and claims data
- WHEN the shell constructs signing input or proof-reference bytes
- THEN the operation MUST be deterministic for the same inputs
- AND tests or future verified helpers MUST avoid printing compact UCAN token values, bearer values, signing secrets, private keys, or credential-like logs

#### Scenario: Runtime trust boundaries are not hidden [r[library.verified-compact-token-codec-boundary.scenario.trust-boundaries]]

- GIVEN compact-token verification that uses runtime codec, hash, and cryptographic libraries
- WHEN documentation or guard scripts describe verification coverage
- THEN they MUST state that codec/hash/crypto internals remain trusted boundaries unless replaced by exact verified primitives
