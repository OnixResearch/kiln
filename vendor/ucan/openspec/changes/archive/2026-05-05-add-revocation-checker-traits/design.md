## Context

Revocation is an authorization policy over token identity. This repository already has a stable proof reference function based on compact-token bytes and a caller-injected resolver/proof-store model. Revocation should compose with that model rather than introducing a process-global registry or network fetcher.

## Goals / Non-Goals

**Goals:** explicit caller-supplied revocation checks, leaf and proof-chain coverage, structured denials, and backwards-compatible default verification.

**Non-Goals:** revocation credential formats, registry protocols, network fetching, persistence layers, replay stores, or domain-specific revocation semantics.

## Decisions

### 1. Key revocation by proof reference

**Choice:** Use `ProofReference` as the revocation subject for compact tokens.

**Rationale:** Proof references are already derived deterministically from compact-token bytes and are used to address proof material. This avoids adding new token IDs or parsing before an initial revocation precheck.

**Alternative:** Key by issuer/subject pairs or embedded claims. Rejected because it requires parsing and domain policy before the generic verifier can deny.

**Implementation:** Add `RevocationChecker::is_revoked(&ProofReference)` plus `RevocationList` for in-memory tests/integrations.

### 2. Check every traversal node

**Choice:** The revocation-aware traversal checks the current token before parse/signature verification and recurses with the same checker for parent proofs.

**Rationale:** A valid child must not inherit authority from a revoked parent. Checking only the leaf would leave a common production footgun.

**Alternative:** Add a post-verification hook on `VerifiedToken`. Rejected because it cannot reliably cover hidden parents unless callers re-walk proofs.

### 3. Keep no-revocation path stable

**Choice:** Existing `verify_compact_token*` functions delegate through a `NoRevocations` checker.

**Rationale:** Revocation state is application-owned and optional; existing callers should not be forced to allocate or change behavior.

## Risks / Trade-offs

**Checker backend failures** → Propagate as structured `TokenError::Revocation` so callers can fail closed.

**False sense of registry support** → Docs explicitly state the library supplies injection points and in-memory lists, not network or persistence semantics.
