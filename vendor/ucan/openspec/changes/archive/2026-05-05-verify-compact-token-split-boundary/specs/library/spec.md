## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]

The library MUST expose and use verified compact-token structural bridges for already-encoded compact token parts while continuing to distinguish trusted runtime codec, hash, and cryptographic boundaries from verified structural admission. The bridges MUST NOT claim base64url, serde JSON, BLAKE3, Ed25519, resolver, proof-store, revocation-backend, caveat-policy, filesystem, network, signing, persistence, or ambient-clock behavior is formally verified.

#### Scenario: Compact split bridge rejects embedded separators [r[library.verified-compact-token-codec-boundary.scenario.compact-split-bridge]]

- GIVEN shell-split encoded header, payload, and signature bytes plus the original compact-token bytes
- WHEN any supplied part contains an embedded compact separator or the compact bytes do not equal exactly `header.payload.signature`
- THEN the verified split bridge MUST reject the split facts
- AND parse-time integration MUST fail closed with a structured compact-token error before base64url or JSON decoding
- AND documentation MUST state that this proves only separator-free three-part byte framing, not runtime codec, hash, or cryptographic correctness
