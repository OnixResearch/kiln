## ADDED Requirements

### Requirement: Verification accepts explicit revocation checkers [r[library.revocation.explicit-checker]]
The library MUST let callers inject revocation state through explicit checker objects without process-global registries, implicit network fetches, persistence layers, or ambient default stores.

#### Scenario: No-op checker preserves current verification [r[library.revocation.explicit-checker.noop-preserves-behavior]]
- GIVEN a compact token and proof chain that verifies successfully with the existing verification API
- WHEN the same token is verified with the no-op revocation checker
- THEN verification MUST produce the same verified issuer, audience, proof list, and effective delegation constraints

#### Scenario: Revocation backend errors fail closed [r[library.revocation.explicit-checker.backend-error-fails-closed]]
- GIVEN a revocation checker that returns a backend error for a token reference
- WHEN verification reaches that reference
- THEN verification MUST fail with a structured revocation backend error instead of allowing the token

### Requirement: Revocation covers the full proof chain [r[library.revocation.chain-denial]]
The library MUST reject a token when the leaf token or any recursively resolved proof-chain token is revoked by the caller-supplied checker.

#### Scenario: Leaf token is revoked [r[library.revocation.chain-denial.leaf-revoked]]
- GIVEN a valid compact token whose proof reference is present in the caller-supplied revocation list
- WHEN revocation-aware verification is run
- THEN verification MUST fail before returning a verified token

#### Scenario: Parent proof token is revoked [r[library.revocation.chain-denial.parent-revoked]]
- GIVEN a child token whose parent proof token is otherwise valid but whose proof reference is revoked
- WHEN revocation-aware verification walks the delegation chain
- THEN verification MUST fail and MUST NOT authorize the child through the revoked parent

## MODIFIED Requirements

### Requirement: Consumer onboarding is part of the library surface
The README and public API docs MUST explain supported verification inputs, including explicit revocation checkers, and must distinguish caller-supplied revocation checks from out-of-scope registry, network, persistence, replay-store, and credential-format features.

#### Scenario: Support limits name revocation model
- GIVEN a consumer reads the support-limits documentation
- WHEN they look for revocation support
- THEN the documentation MUST state that explicit caller-supplied revocation checks are supported and registry/network/storage semantics remain out of scope
