## Why

Compact parsing now verifies compact framing, encoded base64url segment shape, and decoded JSON object envelopes before serde. The next serde-boundary fact used by the shell is header admission: parsed `alg` and `typ` strings must exactly match the supported UCAN header values before signature verification continues. Routing this exact parsed-field admission through verified logic reduces another runtime-only branch while keeping serde field extraction and JSON parser correctness explicit trusted boundaries.

## What Changes

- Add a vendored verified-logic predicate for parsed UCAN header field admission.
- Expose a UCAN bridge for parsed header fields.
- Route `validate_header` through that bridge before structural token validation continues.
- Document that this verifies only shell/serde-supplied parsed field byte equality, not serde correctness, canonicalization, signature verification, or token provenance.

## Impact

- Files: `verified-logic/src`, `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, docs, bridge guard script.
- APIs: new public bridge type/function for parsed header structural admission.
- Testing: sibling Rust/Verus checks, UCAN focused tests, workspace tests, clippy, OpenSpec validation, bridge guards, Nix flake check.
