## Why

The shell currently exposes verified `did:key` encoding for Ed25519 public keys but not the matching vendored decoder. Advanced callers that need deterministic public DID-key admission must either reimplement prefix/base58/multicodec parsing or reach into the vendored snapshot directly.

## What Changes

- Add a public data-only decoded DID-key wrapper and `decode_did_key` bridge over `verified_logic::ucan_did::decode_did_key`.
- Re-export the helper from the crate root and strengthen the bridge guard.
- Document that the helper performs deterministic structural decoding only, not DID resolution, key lookup, network access, or cryptographic trust decisions.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, docs, bridge guard, OpenSpec.
- **APIs**: new advanced public helper for decoded DID-key bytes and multicodec value.
- **Testing**: focused decode round-trip/negative tests plus standard UCAN gates.
