## Context

The current token issuer owns an `Ed25519SigningKey` value and examples construct it from fixed seed bytes. That is a good deterministic test path, but it is not the right default boundary for reusable crates. The issue pipeline should need only a DID and a signature, while key custody remains behind a signer abstraction.

## Goals / Non-Goals

**Goals:** prevent accidental secret formatting leaks, make signing pluggable, preserve the existing Ed25519 path, and keep signer side effects in the std shell.

**Non-Goals:** implement external KMS/HSM providers, add new signature algorithms, or change compact-token claim semantics.

## Decisions

### 1. Define a shell-owned signer abstraction

**Choice:** Add a public signer interface with methods for issuer DID, algorithm identity, and signing bytes.

**Rationale:** Issuance needs identity plus signatures, not raw private keys. A trait boundary lets downstream crates provide remote signers later without changing `issue_token` semantics.

**Alternative:** Keep accepting concrete `Ed25519SigningKey` only. Rejected because it bakes in local key custody.

**Implementation:** Introduce signer-specific result/error types and route token signing through the abstraction. Keep all verifier semantics in `verified-logic`.

### 2. Keep Ed25519 as the first concrete signer adapter

**Choice:** Rename or wrap the existing signing key as an in-memory Ed25519 signer implementing the new abstraction.

**Rationale:** This preserves current examples/tests while making it explicit that the concrete type is one adapter.

**Alternative:** Replace Ed25519 types entirely. Rejected because current API already works and should remain migratable.

**Implementation:** Provide constructors for deterministic test seeds and, if dependency review accepts it, RNG-backed generation. Keep public key and DID derivation methods.

### 3. Redact secret material at type boundaries

**Choice:** Implement manual formatting for secret-bearing types and add regression tests that search formatted output for known secret values.

**Rationale:** Secret leaks through `Debug` are high-impact and easy to miss in test failures or logs.

**Alternative:** Rely on caller discipline. Rejected because safe reusable libraries need safe defaults.

**Implementation:** Remove derived `Debug` from secret-bearing types or replace it with redacted implementations. Consider `zeroize` for memory cleanup if dependency policy accepts it.

## Risks / Trade-offs

**Trait object vs generic signer ergonomics** → Prefer a simple borrowed trait API first; add boxed/object-safe wrappers only if needed by examples.

**Dependency creep for key generation/zeroize** → Keep generation optional if necessary; redaction is mandatory.

**Migration churn** → Keep compatibility constructors or a clear alias during the transition.
