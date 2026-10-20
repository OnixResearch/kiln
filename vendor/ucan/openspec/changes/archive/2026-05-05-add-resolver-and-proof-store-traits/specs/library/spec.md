## ADDED Requirements

### Requirement: Verifiers use explicit key resolvers

The `ucan` shell MUST support verification through an explicit key resolver abstraction rather than only a concrete in-memory vector.
r[library.resolver-proof-store.key-resolver]

#### Scenario: In-memory resolver verifies existing tokens
r[library.resolver-proof-store.key-resolver.in-memory-resolver-verifies-existing-tokens]

- GIVEN a token issued by an Ed25519 signer and an in-memory resolver containing the issuer verification key
- WHEN the token is verified through the resolver-based context
- THEN verification MUST succeed
- AND the result MUST match the current in-memory-key behavior

#### Scenario: Missing issuer key is structured
r[library.resolver-proof-store.key-resolver.missing-issuer-key-is-structured]

- GIVEN a token whose issuer key is not available from the resolver
- WHEN verification attempts to resolve the issuer
- THEN verification MUST fail with a structured missing-key error that includes the issuer DID

### Requirement: Proof traversal uses explicit proof stores

The verifier MUST support resolving proof tokens through a proof store abstraction instead of requiring every proof token to be pre-collected in one vector.
r[library.resolver-proof-store.proof-store]

#### Scenario: Proof store supplies parent token by reference
r[library.resolver-proof-store.proof-store.supplies-parent-by-reference]

- GIVEN a child token whose proof reference points to a parent token
- AND a proof store that can return the parent token for that reference
- WHEN the child token is verified
- THEN verification MUST resolve the parent through the proof store
- AND proof-chain validation MUST succeed when linkage, time, and attenuation are valid

#### Scenario: Missing proof is structured
r[library.resolver-proof-store.proof-store.missing-proof-is-structured]

- GIVEN a child token whose proof reference is not available from the proof store
- WHEN verification attempts to resolve proofs
- THEN verification MUST fail with a structured missing-proof error
- AND the verifier MUST NOT silently ignore the missing proof

### Requirement: Recursive verification has deterministic limits

Proof-chain verification MUST enforce explicit limits for traversal depth, proof count, and serialized compact-token byte size. The token-size limit MUST apply to the input token and to each resolved proof token before parsing.
r[library.resolver-proof-store.verification-limits]

#### Scenario: Serialized token size limit rejects oversized input
r[library.resolver-proof-store.verification-limits.serialized-token-size-limit]

- GIVEN an input token or resolved proof token whose compact string byte length exceeds the configured maximum
- WHEN verification attempts to process that token
- THEN verification MUST fail with a structured limit-exceeded error
- AND the oversized token MUST NOT be parsed or trusted

#### Scenario: Depth limit stops recursive traversal
r[library.resolver-proof-store.verification-limits.depth-limit-stops-recursion]

- GIVEN a proof chain deeper than the configured maximum depth
- WHEN verification traverses proofs
- THEN verification MUST fail with a structured limit-exceeded error
- AND the verifier MUST NOT continue unbounded recursion

#### Scenario: Proof-count limit stops broad traversal
r[library.resolver-proof-store.verification-limits.proof-count-limit-stops-traversal]

- GIVEN a token or chain whose total resolved proof count exceeds the configured maximum proof count
- WHEN verification traverses proofs
- THEN verification MUST fail with a structured limit-exceeded error
- AND the verifier MUST NOT continue resolving additional proofs

#### Scenario: Cycle detection remains enforced with stores
r[library.resolver-proof-store.verification-limits.cycle-detection-with-proof-store]

- GIVEN proof tokens that form a reference cycle through the proof store
- WHEN verification traverses the chain
- THEN verification MUST reject the cycle with a structured proof-chain error

### Requirement: Resolver/store abstractions preserve the core/shell boundary

Key resolver and proof store abstractions MUST live in the `ucan` std shell and MUST NOT add storage, network, clock, resolver, or codec dependencies to `ucan-core`.
r[library.resolver-proof-store.fcis-boundary]

#### Scenario: Core remains independent of resolver/store implementations
r[library.resolver-proof-store.fcis-boundary.core-independent-of-resolvers]

- GIVEN resolver and proof-store traits in the shell
- WHEN `ucan-core` is built and audited
- THEN it MUST remain free of resolver, storage, network, filesystem, clock, JSON, and compact-token codec dependencies

### Requirement: Resolver/store onboarding is documented

The repository MUST document resolver/store injection and provide at least one runnable example using a simple proof store.
r[library.resolver-proof-store.consumer-onboarding]

#### Scenario: Proof-store example resolves a parent token
r[library.resolver-proof-store.consumer-onboarding.proof-store-example]

- GIVEN the proof-store example
- WHEN it runs through the public crate API
- THEN it MUST resolve a parent token by proof reference and verify the child token without private modules
