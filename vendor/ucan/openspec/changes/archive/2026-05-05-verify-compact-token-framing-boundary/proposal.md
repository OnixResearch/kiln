## Why

Compact UCAN verification currently constructs canonical signing input and compact framing in ordinary Rust before runtime base64url, JSON, signature, and hash boundaries. A small verified-logic primitive can close the byte-framing portion without claiming to verify codec or cryptographic internals.

## What Changes

- Add a UCAN bridge for verified compact-token framing over already-encoded header, payload, and signature parts.
- Route shell parsing through the bridge after compact splitting and before decoded-token construction.
- Document that base64url decoding, JSON serde, BLAKE3 references, and Ed25519 verification remain trusted runtime boundaries.

## Capabilities

### Modified Capabilities
- `library`: compact-token parsing/signing-input framing has explicit verified structural coverage.

## Impact

- **Files**: `src/verified.rs`, `src/token.rs`, docs, bridge guard, vendored `verified-logic`.
- **APIs**: Adds an explicit compact framing bridge type/function.
- **Dependencies**: No new runtime dependencies.
- **Testing**: Cargo tests, bridge guard, no-std guard, OpenSpec validation, and Nix flake check.
