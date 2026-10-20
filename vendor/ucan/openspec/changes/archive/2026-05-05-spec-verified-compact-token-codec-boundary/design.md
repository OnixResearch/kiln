## Context

The shell currently handles JWT/JWS-style compact string splitting, JSON serialization, base64url encoding/decoding, signing-input construction, and BLAKE3 proof-reference derivation. Vendored `verified-logic` keeps codec/hash/crypto internals as explicit trusted/runtime boundaries.

## Goals / Non-Goals

**Goals:** document a future path for structural codec admission checks, deterministic signing-input construction tests, and boundary guards.

**Non-Goals:** no claim that base64url, serde JSON, BLAKE3, or Ed25519 internals become formally verified in this crate unless new verified primitives are added upstream.

## Decisions

### 1. Separate structural admission from codec correctness

**Choice:** Future verification MAY prove compact part counts, non-empty component admission, and stable assembly rules while continuing to treat codec/hash internals as trusted boundaries.

**Rationale:** This closes what can be structurally checked without overclaiming cryptographic or codec proof coverage.

**Alternative:** Treat existing runtime codec libraries as verified by tests. Rejected because tests are not machine-checked formal proof.

## Risks / Trade-offs

**Overclaiming verification coverage** → Docs and spec MUST distinguish structural admission from trusted codec/hash/crypto implementations.
