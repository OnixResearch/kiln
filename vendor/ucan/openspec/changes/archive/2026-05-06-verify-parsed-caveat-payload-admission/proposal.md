## Why

Decoded caveat payloads cross a codec/parser boundary before `ucan-core` rejects empty payload bytes. The shell already verifies caveat field presence and delegates opaque semantics to caller policies, but payload string admission is still ordinary Rust around base64url decoding.

## What Changes

- Add a narrow verified admission seam for parsed caveat payload facts: payload field presence, encoded non-empty, encoded base64url-no-pad admission, and decoded non-empty bytes.
- Route decoded caveat payload validation through the vendored verified bridge before constructing core caveat inputs.
- Preserve existing caveat policy and parser/codec trust boundaries.

## Capabilities

### Modified Capabilities
- `library.compact-token`: caveat payloads gain a verified finite shape/admission predicate before core structural acceptance.

## Impact

- **Files**: `verified-logic` UCAN primitive module, UCAN vendor snapshot, `src/verified.rs`, `src/token.rs`, docs, guard script, OpenSpec archive.
- **APIs**: new public bridge facts/functions for parsed caveat payload admission.
- **Dependencies**: no new external dependencies.
- **Testing**: sibling primitive tests/verification plus UCAN focused/full gates and `nix flake check`.
