# library Specification Delta

## Modified Requirements

### Requirement: Delegation proof chains are verified end to end

The library MUST verify delegation proof chains across parent tokens, including issuer/audience linkage, temporal narrowing, and capability attenuation. Issuer/audience proof-link admission MUST route through the vendored verified-chain bridge rather than a shell-local direct semantic comparison. Proof-reference construction from shell-computed digest bytes MUST route through a vendored verified proof-reference digest admission bridge before the reference is used for proof-store lookup, cycle detection, traversal transcripts, or revocation checks.

#### Scenario: Proof reference digest admission uses verified bridge [r[library.delegation-chain.proof-reference-digest-admission-uses-verified-bridge]]

- GIVEN compact token bytes and the shell-computed proof-reference digest bytes
- WHEN the shell constructs a proof reference for proof traversal, proof-store lookup, cycle detection, or revocation checks
- THEN the digest bytes MUST be admitted through the vendored verified proof-reference bridge
- AND admission MUST require the digest to have the supported fixed digest length
- AND admission MUST require the proof-reference bytes to exactly match the digest bytes
- AND the bridge MUST NOT claim to verify BLAKE3 correctness, compact-token canonicalization, token provenance, base64url decoding, JSON parsing, signature verification, backend truth, or revocation freshness
