## Context

The current UCAN shell routes several compact-token admission facts through vendored verified-logic bridges. This change keeps that pattern: close only a finite deterministic fact emitted by serde/shell code and leave parser, crypto, backend, freshness, and provenance as explicit trust boundaries.

## Goals / Non-Goals

**Goals:**
- Add one narrow verified admission seam for `verify-verification-stage-token-flow`.
- Keep the bridge deterministic and data-only.
- Add tests, documentation, and bridge-guard coverage for the new seam.

**Non-Goals:**
- Prove base64url, JSON, Ed25519, BLAKE3, DID resolution, backend truth, registry freshness, clock provenance, network/storage behavior, or arbitrary caveat semantics.
- Add ambient resolvers, stores, clocks, async runtimes, replay stores, or process-global fallback behavior.

## Decisions

### 1. Use a narrow vendored bridge

**Choice:** Model only the finite shell-supplied facts named by this change and expose them through a small UCAN bridge.

**Rationale:** This keeps the verified claim exact and reviewable while shrinking the ordinary Rust authorization surface.

**Alternative:** Prove the whole token parser/verifier flow in one change. Rejected because parser, crypto, and backend boundaries are external and would force overclaims.

### 2. Preserve fail-closed behavior

**Choice:** Failed admission rejects before the downstream stage consumes the value.

**Rationale:** Verified admission is only useful if malformed or inconsistent shell facts cannot proceed into authorization.

## Risks / Trade-offs

- **Overclaim risk** → Documentation and tests must name the non-claims explicitly.
- **Bridge drift** → `scripts/check-ucan-bridge.sh` must require the vendored primitive, UCAN bridge, public export if applicable, and live integration.
- **Archive clobber risk** → Inspect canonical `openspec/specs/library/spec.md` after archive before committing implementation slices.
