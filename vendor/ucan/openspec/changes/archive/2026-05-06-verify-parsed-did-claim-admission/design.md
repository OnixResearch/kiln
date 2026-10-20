## Context

`validate_decoded_token` already checks parsed required-claim presence through `VerifiedParsedClaimsPresence` and numeric temporal order through `VerifiedParsedTemporalOrder`. It then constructs `IssuerDid`, `AudienceDid`, and core claims using ordinary Rust validators that reject empty or untrimmed DID strings.

## Goals

- Admit the deterministic parsed issuer/audience DID shape predicate through vendored verified logic.
- Preserve the existing core errors and wrapper validation behavior for callers.
- Keep the seam finite/data-only and based on shell-supplied facts.

## Non-Goals

- Proving serde JSON parser correctness, UTF-8/string extraction, duplicate-key behavior, canonicalization, DID method semantics, DID-key derivation, key-resolution truth, signature correctness, token provenance, or resolver/backend behavior.
- Moving DID validation into `ucan-core` Verus or changing public DID wrapper semantics.

## Decisions

### 1. Boolean fact bridge

**Choice:** The verified primitive accepts booleans for issuer/audience present, non-empty, and trimmed.

**Rationale:** This exactly mirrors the finite admission predicate available at the shell boundary while leaving string trimming/provenance and parser behavior explicit.

### 2. Live path before core construction

**Choice:** `validate_decoded_token` checks parsed DID facts before constructing wrappers/core claims.

**Rationale:** This keeps failures fail-closed through the verified primitive and demonstrates the seam in the production validation path.

## Risks / Trade-offs

- The shell still computes the booleans with ordinary Rust string methods; documentation must avoid overclaiming string/parser correctness.
- Existing direct `IssuerDid::new` / `AudienceDid::new` wrappers remain ordinary Rust validation surfaces for ergonomic APIs.
