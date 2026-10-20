## Why

Revocation-aware verification is implemented, but consumers need a runnable example that demonstrates the supported in-memory `RevocationList` flow without exposing compact token values or implying registry/network semantics.

## What Changes

- Add a runnable `revocation` example that verifies a valid child token, then rejects the same chain when the parent proof reference is revoked.
- Update consumer docs to point to the example.

## Impact

- **Files**: `examples/revocation.rs`, README, public API docs, library OpenSpec.
- **APIs**: No new API surface.
- **Testing**: Build/run the new example plus standard Rust/OpenSpec gates.
