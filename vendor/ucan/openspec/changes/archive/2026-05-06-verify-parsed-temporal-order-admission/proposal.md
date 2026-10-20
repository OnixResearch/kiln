## Why

Parsed UCAN claim admission now verifies required field presence before core claim construction, but the semantic relation between parsed `nbf` and `exp` still reaches ordinary core validation directly. The exact closeable fact is finite and data-only: a shell/serde-supplied not-before timestamp must be strictly earlier than the expiration timestamp before a token time window is accepted.

## What Changes

- Add a vendored verified temporal-order primitive for parsed UCAN claim timestamps.
- Route decoded `nbf`/`exp` ordering through a UCAN verified bridge before constructing core claim input.
- Preserve parser, numeric deserialization, clock freshness/provenance, expiration-at-now checks, proof traversal, signing-input, and cryptographic boundaries as non-claims.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, public exports/docs/guards, and this OpenSpec package.
- **APIs**: add a small public parsed temporal-order bridge type/function.
- **Testing**: focused sibling tests/Verus, focused UCAN bridge/integration tests, UCAN workspace checks, bridge guard, OpenSpec validation, and Nix flake check.
