## Context

`web_of_trust_ucan_authorization` accepts finite caller-supplied principal digest byte vectors for trusted roots, attestations, revocations, and subject facts. The shell currently rejects non-32-byte digests and constructs vendored `PrincipalId` values directly. The vendored primitive `web_of_trust_principal::principal_id_from_digest_bytes` already expresses the exact raw digest admission rule.

## Goals / Non-Goals

**Goals:**
- expose a small public predicate for caller-side digest preflight;
- use the vendored constructor when converting raw WOT digest bytes to `PrincipalId`;
- preserve existing structured `InvalidPrincipalDigestLength` errors for WOT authorizer conversion failures.

**Non-Goals:**
- no DID resolution, trust-record fetching, attestation validation, revocation freshness/backend checks, UCAN authorization, persistence, networking, signing, replay protection, or ambient clock behavior;
- no replacement of `web_of_trust_principal_digest` derivation from public key bytes;
- no new opaque principal wrapper type or serialization surface.

## Decisions

### 1. Boolean digest-admission helper

**Choice:** add `web_of_trust_principal_digest_is_valid(digest: &[u8]) -> bool`.

**Rationale:** callers can preflight raw digest bytes before building WOT request structures, and the helper maps exactly to the vendored admission primitive's `Some`/`None` result.

**Alternative:** expose the vendored `PrincipalId` type or introduce a new wrapper. Rejected for this slice because the existing WOT public API is byte-vector based and adding a wrapper would broaden ergonomics/serde decisions beyond the exact verified predicate.

### 2. Internal conversion through vendored constructor

**Choice:** change `build_web_of_trust_principal` to call `principal_id_from_digest_bytes(digest.to_vec())` and map `None` to the existing structured length error.

**Rationale:** this removes a shell-local reimplementation of raw digest admission while keeping the shell's public error type stable.

## Risks / Trade-offs

**Small helper overlap** → The helper is intentionally narrow and overlaps with length checking; documentation and tests must state that this is only fixed-width digest admission, not WOT evidence validation or authorization.

## Validation Plan

Run focused tests for valid/invalid digest lengths and WOT authorizer error preservation, then the standard UCAN gate sequence: fmt, tests, clippy, no-std guard, bridge guard, strict OpenSpec validation, `git diff --check`, and Nix flake check from `/tmp`.
