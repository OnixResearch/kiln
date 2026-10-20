## Why

Parsed proof-list presence is admitted, but each parsed `prf` entry still reaches proof-reference decoding as ordinary shell data. The closeable fact is finite: every shell/serde-supplied encoded proof reference entry should satisfy the compact base64url-no-pad admission rules before decoding or proof input construction.

## What Changes

- Add or reuse a vendored verified primitive for parsed proof-reference list entry shape.
- Route each parsed `prf` entry through a UCAN bridge before proof-reference decoding and core proof input construction.
- Preserve base64 decoder, decoded-byte, BLAKE3 derivation, proof truth, backend provenance, and cryptographic boundaries as non-claims.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, docs, bridge guard, tests, and this OpenSpec package.
- **APIs**: add a small parsed proof-reference list-shape bridge type/function.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
