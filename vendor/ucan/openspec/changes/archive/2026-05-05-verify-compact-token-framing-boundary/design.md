## Context

The std shell owns compact string splitting, base64url decoding, JSON serde, signing-input construction, signature verification, and proof-reference hashing. Vendored `verified-logic` now has a data-only compact framing primitive that accepts already-encoded byte parts and caller-supplied compact/signing-input bytes.

## Goals / Non-Goals

**Goals:**
- Prove/bridge exact `header.payload.signature` framing and `header.payload` signing-input bytes.
- Use the bridge in `parse_compact_token` after the existing split step.
- Keep all codec/hash/crypto parsing behavior in the shell.

**Non-Goals:**
- Verifying base64url, JSON serde, BLAKE3 proof-reference derivation, Ed25519 cryptography, resolver/proof-store/revocation behavior, or caveat policy semantics.
- Exposing compact UCAN token values or signing secrets in logs/docs/tests.

## Decisions

### 1. Already-encoded parts boundary

**Choice:** The bridge consumes already-split encoded header/payload/signature byte vectors plus the compact and signing-input byte vectors.
**Rationale:** This exactly matches the newly verified primitive while preserving shell codec boundaries.
**Alternative:** Reimplement compact splitting inside verified-logic; rejected because base64/JSON/string parsing is a separate larger boundary.

### 2. Parse-time bridge call

**Choice:** `parse_compact_token` checks the compact envelope before decoding JSON/signature bytes.
**Rationale:** The split parts and signing input are already available there, so the check is deterministic and side-effect free.

## Risks / Trade-offs

**Scope overclaiming** → Mitigated by docs and guard wording that name codec/hash/crypto internals as trusted boundaries.

**Secret/token exposure** → Tests use synthetic byte fixtures and MUST NOT print real compact UCAN token values.
