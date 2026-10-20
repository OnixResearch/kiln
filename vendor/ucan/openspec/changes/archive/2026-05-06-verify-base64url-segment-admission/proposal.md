# Verify base64url segment admission

## Why

Compact UCAN parsing still passes encoded header, payload, and signature strings directly to the runtime base64url decoder. The decoder remains a trusted codec boundary, but the shell can fail closed earlier on exact structural facts: every encoded segment must use the URL-safe no-padding alphabet and must not have impossible unpadded length class.

## What Changes

- Add a vendored verified-logic primitive for base64url-no-pad segment admission.
- Expose a UCAN bridge helper and use it before runtime base64url decoding.
- Keep runtime base64url decoding, decoded-byte correctness, JSON serde, BLAKE3, and Ed25519 outside the verified claim.

## Impact

- Affected code: `verified-logic`, `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, docs, and bridge guards.
- Verification: sibling Rust/Verus checks, UCAN cargo/Nix checks, OpenSpec strict validation, and archive.
