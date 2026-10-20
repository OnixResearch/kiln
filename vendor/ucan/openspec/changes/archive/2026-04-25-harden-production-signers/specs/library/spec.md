## ADDED Requirements

### Requirement: Secret key material is redacted from formatting

The `ucan` shell MUST ensure secret signing material is not exposed through `Debug`, display, error messages, or test failure output.
r[library.production-signers.secret-redaction]

#### Scenario: Debug output redacts in-memory signer secrets
r[library.production-signers.secret-redaction.debug-redacts-in-memory-secrets]

- GIVEN an in-memory Ed25519 signer initialized with known secret bytes
- WHEN a caller formats the signer or secret-key wrapper with `Debug`
- THEN the formatted output MUST NOT contain the secret bytes, seed bytes, or a direct byte-array representation
- AND the output SHOULD identify the type using a redacted placeholder

#### Scenario: Invalid key errors do not echo secret material
r[library.production-signers.secret-redaction.invalid-key-errors-do-not-echo-secrets]

- GIVEN malformed secret key input
- WHEN construction fails
- THEN the error MUST describe the validation failure without embedding the provided secret bytes

### Requirement: Token issuance is driven by a signer abstraction

The public token issuance path MUST accept a signer abstraction that provides issuer identity and signatures without requiring callers to expose raw secret bytes to the issuance function.
r[library.production-signers.signer-abstraction]

#### Scenario: In-memory signer issues the same token shape
r[library.production-signers.signer-abstraction.in-memory-signer-issues-token]

- GIVEN an in-memory Ed25519 signer, audience DID, capabilities, proofs, and valid time bounds
- WHEN a caller issues a token through the signer-based API
- THEN the token MUST use the existing compact-token claim shape
- AND verification through the public verifier MUST succeed

#### Scenario: Signer failure returns structured issue error
r[library.production-signers.signer-abstraction.signer-failure-is-structured]

- GIVEN a signer implementation that refuses to sign
- WHEN a caller issues a token through the signer-based API
- THEN issuance MUST fail with a structured signer-related error
- AND the library MUST NOT panic

#### Scenario: Bad signer output is rejected
r[library.production-signers.signer-abstraction.bad-signer-output-is-rejected]

- GIVEN a signer implementation that reports an unsupported algorithm, returns malformed signature bytes, or reports an issuer identity inconsistent with its verification material
- WHEN a caller issues or verifies a token through the signer-based API
- THEN the library MUST fail with a structured signer, signature, or issuer-mismatch error
- AND the library MUST NOT treat the token as valid

### Requirement: Ed25519 signer adapter preserves verified semantics

The existing Ed25519 behavior MUST remain available as a concrete signer adapter, and its DID derivation, signing, and verification MUST continue to route through `vendor/verified-logic`.
r[library.production-signers.ed25519-adapter]

#### Scenario: Ed25519 signer issuer matches derived did:key
r[library.production-signers.ed25519-adapter.issuer-matches-derived-did-key]

- GIVEN an Ed25519 signer adapter
- WHEN the library derives its issuer DID and issues a token
- THEN the issuer claim MUST match the verified `did:key` derived from the signer public key
- AND signature verification MUST succeed through the verified-logic bridge

### Requirement: Signer APIs preserve the core/shell boundary

Signer construction, key custody, randomness, and signing MUST remain in the `ucan` std shell, while `ucan-core` remains free of signer, key-generation, clock, filesystem, network, JSON, and compact-token codec dependencies.
r[library.production-signers.fcis-boundary]

#### Scenario: Core remains independent of signer implementation
r[library.production-signers.fcis-boundary.core-independent-of-signer]

- GIVEN the signer abstraction and Ed25519 signer adapter
- WHEN `ucan-core` is built and audited
- THEN it MUST not depend on std-only signer, randomness, key-storage, JSON, filesystem, network, or clock concerns

### Requirement: Examples and docs teach the signer boundary

The repository MUST update affected examples and public docs so consumers see signer-based issuance as the production-oriented path.
r[library.production-signers.consumer-onboarding]

#### Scenario: Existing examples use signer-based issuance where appropriate
r[library.production-signers.consumer-onboarding.examples-use-signer-api]

- GIVEN the runnable `issue_and_verify` and `proof_chain` examples
- WHEN the signer abstraction is available
- THEN examples SHOULD demonstrate the signer-based issuance path instead of encouraging direct raw secret-key handling as the primary reusable-crate pattern
