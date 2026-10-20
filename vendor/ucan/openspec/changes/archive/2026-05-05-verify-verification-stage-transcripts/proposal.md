## Why

UCAN verification now has verified helpers for compact byte framing and finite proof traversal facts, but the high-level parse/signature/proof-load/error boundary is still described only as ordinary Rust control flow. A small finite transcript bridge can make the shell's fail-closed stage admission explicit without claiming to verify codec, cryptography, resolver, proof-store, revocation freshness, or caveat policy internals.

## What Changes

- Add a data-only verified verification-stage transcript primitive to vendored `verified-logic`.
- Expose a UCAN bridge API for caller/shell-supplied stage facts.
- Document that acceptance requires every stage fact to be positive and that rejection identifies the first failed stage.
- Keep runtime parsing, base64url/JSON, Ed25519, BLAKE3, resolver/proof-store/revocation backends, proof material provenance, and caveat policy semantics outside scope.

## Impact

- Files: `vendor/verified-logic`, `src/verified.rs`, docs, guard script, OpenSpec library spec.
- APIs: add verification-stage transcript bridge types/functions.
- Testing: verified-logic Verus/Rust tests, UCAN bridge tests, OpenSpec validation, Rust/Nix gates.
