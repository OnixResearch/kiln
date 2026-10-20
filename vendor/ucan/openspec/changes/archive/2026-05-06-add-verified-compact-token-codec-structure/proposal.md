## Why

The shell already uses finite compact-token bridge predicates, but some input facts such as JSON significant-byte indexes and signing-input provenance are still supplied by ordinary Rust. Verifying more codec structure reduces parser/provenance trust without claiming base64url, serde, or crypto correctness.

## What Changes

- **1**: Add verified predicates for exact compact-token segment count and signing-input reconstruction.
- **2**: Move decoded JSON object-envelope significant-byte index discovery into verified logic where possible.
- **3**: Document base64url, serde, canonicalization, and cryptography as unchanged boundaries.

## Capabilities

### New Capabilities
- `add-verified-compact-token-codec-structure`: Specify verified compact-token codec structure and signing-input provenance predicates.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
