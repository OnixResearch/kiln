## Why

The compact-token bridge already verifies canonical `header.payload.signature` and `header.payload` byte framing after the shell has split the token. The split boundary itself is still described as a runtime trust boundary even though one exact structural fact is closeable: already-split compact parts should contain no embedded compact separators, and the whole compact byte string should be exactly those three parts separated by two dots.

## What Changes

- Add a verified `verified-logic` predicate for separator-free three-part compact-token split admission.
- Sync the vendored snapshot into `ucan` and expose a public bridge helper.
- Route `parse_compact_token` through the new split-admission helper after shell splitting and before base64url/JSON decoding.
- Update tests, docs, and bridge guards to distinguish verified split/framing facts from remaining runtime base64url, JSON, BLAKE3, and Ed25519 boundaries.

## Scope

In scope: finite byte-level separator admission for already-split compact token parts and parse-time fail-closed integration.

Out of scope: proving Rust `str::split`, base64url decoding, JSON serde/canonicalization, BLAKE3 proof-reference derivation, Ed25519 internals, resolver/proof-store/revocation truth, or arbitrary compact-token parser correctness beyond the checked byte facts.

## Verification

Run sibling Rust/Verus checks for the new primitive, sync vendor, then run UCAN fmt/tests/clippy/no-std/bridge/OpenSpec/diff/Nix gates before archive and push.
