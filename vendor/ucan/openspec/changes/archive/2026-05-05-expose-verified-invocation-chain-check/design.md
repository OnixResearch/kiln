## Context

Recent UCAN work exposed separate verified bridge helpers for structural chains, non-revoked chains, concrete invocation admission, revocation membership, proof links, proof depth, and receipts. The vendored `verified_logic::ucan_invocation::check_invocation_chain_authorized` composes a resolved chain, finite revocation set, authorization CID binding, leaf capability index, invoker/audience match, resource prefix refinement, and ability equality.

## Goals / Non-Goals

**Goals:**
- Expose the vendored composed checker as a focused public helper for advanced callers with already-resolved public data.
- Preserve deterministic, caller-supplied inputs and structured conversion errors.
- Add tests for accepted authorization, revoked proof reference, invalid chain, wrong authorization reference, resource/ability escalation, and out-of-bounds leaf capability index.

**Non-Goals:**
- No production replacement for `verify_invocation` or `verify_compact_token_with_resolvers_and_revocations`.
- No proof resolution, compact-token parsing, cryptographic signature verification, DID/key lookup, revocation backend/freshness checks, caveat policy evaluation, wildcard containment replacement, registry/storage/network behavior, persistence, replay checks, or ambient clock reads.

## Decisions

### 1. Data-only public bridge

**Choice:** Add `invocation_chain_is_authorized(...) -> VerifiedResult<bool>` to `src/verified.rs` and re-export it.

**Rationale:** The shell can safely translate existing `VerifiedChainEntry` and byte/string inputs into the verified primitive without adding runtime authority or hidden resolution behavior.

**Alternative:** Wire the composed primitive into `verify_invocation` directly. Rejected because the production path already owns token/proof verification, wildcard/effective delegation semantics, caveat policy, and fail-closed revocation backends. The vendored primitive expects already-resolved finite data and should not weaken those shell-owned checks.

### 2. Explicit authorization reference binding

**Choice:** The helper accepts `authorization_reference` bytes separately from the aligned `chain_references` vector and passes them as `Invocation.auth_cid`.

**Rationale:** The vendored primitive requires the invocation authorization CID/reference to equal the resolved leaf chain reference. Keeping it explicit preserves the review-found binding and lets tests cover wrong-reference denial.

## Risks / Trade-offs

**API arity** → The helper is intentionally explicit rather than hiding context in a builder. Documentation labels it as an advanced bridge.

**Semantic overlap** → Docs and guard comments state this helper is not a production verifier and must not replace caveat/wildcard/fail-closed shell paths.
