## Why

Compact-token parsing, base64url/JSON codec behavior, signing-input construction, and proof-reference hashing remain ordinary shell/runtime trust boundaries. The audit found no current vendored primitive that fully verifies this codec stack, so the next step is to specify the boundary before attempting any implementation.

## What Changes

- Specify which compact-token codec behaviors may be structurally verified in future and which lower-level codec/hash/crypto pieces remain trusted.
- Require future implementation to preserve the current FCIS split: `ucan-core` owns deterministic claim structure, while the shell owns compact serialization and runtime codecs.
- Define negative tests and guard expectations for malformed compact tokens and signing-input stability.

## Capabilities

### New Capabilities
- `library.verified-compact-token-codec-boundary`: Future codec hardening has explicit structural/admission requirements without claiming to verify base64url, serde, BLAKE3, or Ed25519 internals.

## Impact

- **Files**: OpenSpec library spec now; future work may touch `src/token.rs`, `crates/ucan-core`, docs, and tests.
- **APIs**: No immediate API change required.
- **Dependencies**: No new runtime dependencies.
- **Testing**: Future tests must cover malformed compact shape, invalid base64/JSON, signing-input stability, and proof-reference determinism boundaries without printing tokens.
