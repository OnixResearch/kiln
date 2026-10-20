## Why

Compact UCAN parsing now verifies encoded compact framing and base64url-no-pad segment shape before runtime decoding, but decoded header and payload bytes still flow directly into serde JSON. A small structural admission check can reject decoded non-object JSON before serde-derived header/claims semantics, while keeping serde parsing and canonicalization explicit runtime trust boundaries.

## What Changes

- Add a vendored verified-logic predicate for decoded JSON object byte admission over caller/shell-supplied first/last significant byte indexes.
- Expose a UCAN bridge for decoded JSON object admission.
- Route compact header and payload JSON decode through that bridge before serde parsing.
- Document that this verifies only object-envelope byte shape, not JSON parser correctness, canonicalization, fields, signatures, hashes, or token provenance.

## Impact

- Files: `verified-logic/src`, `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, docs, bridge guard script.
- APIs: new public bridge type/function for JSON object structural admission.
- Testing: sibling Rust/Verus checks, UCAN focused tests, workspace tests, clippy, OpenSpec validation, bridge guards, Nix flake check.
