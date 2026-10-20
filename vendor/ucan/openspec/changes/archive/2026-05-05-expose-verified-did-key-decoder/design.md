## Context

`verified_logic::ucan_did` already verifies `decode_did_key` against the vendored DID-key prefix, base58btc, varint multicodec, and public-key byte extraction specs. The UCAN shell only exposes the encoder.

## Decisions

### Data-only public wrapper

**Choice:** Expose a small `VerifiedDidKey` struct with `public_key_bytes`, `codec`, and `encoded` fields, and a `decode_did_key(&[u8]) -> Option<VerifiedDidKey>` helper.

**Rationale:** This mirrors the vendored `DidKey` shape without exposing vendored types as part of the shell API. `Option` preserves the vendored admission semantics: invalid prefix/base58/varint forms return `None`; structurally decoded keys return public bytes and codec.

### Boundary

The helper is only a deterministic structural decoder. It must not perform remote DID resolution, key lookup, cryptographic signature verification, trust-chain checks, proof fetching, filesystem/network fallback, persistence, or ambient clock behavior.

## Risks

- **API overclaim:** Mitigated by documenting the helper as structural/public-data only.
- **Guard drift:** Mitigated by requiring the decode bridge in `scripts/check-ucan-bridge.sh`.
