## ADDED Requirements

### Requirement: Verified non-revoked structural chain bridge [r[library.verified-bridges.nonrevoked-structural-chain]]

The library MUST expose a focused bridge that accepts caller-supplied resolved UCAN chain entries, aligned proof-reference bytes, and finite revoked-reference bytes, then routes structural chain validity plus non-revocation admission through the vendored verified revocation-chain checker.

#### Scenario: Non-revoked structural chain bridge accepts an aligned clean chain [r[library.verified-bridges.nonrevoked-structural-chain.accepts-clean-chain]]

- GIVEN caller-supplied chain entries that are structurally valid at the supplied verification time
- AND the proof-reference vector has exactly one reference for each chain entry
- AND none of those references is present in the finite revoked-reference set
- WHEN the composed non-revoked-chain bridge is evaluated
- THEN it MUST return an allowed result from the verified revocation-chain checker.

#### Scenario: Non-revoked structural chain bridge rejects revoked, misaligned, or invalid chains [r[library.verified-bridges.nonrevoked-structural-chain.rejects-invalid-chain]]

- GIVEN caller-supplied chain entries with a revoked reference, mismatched reference count, or invalid structure
- WHEN the composed non-revoked-chain bridge is evaluated
- THEN it MUST return a rejected result from the verified revocation-chain checker
- AND it MUST NOT invoke proof resolution, network lookup, revocation registries, backend availability checks, caveat policy, or cryptographic signature verification.
