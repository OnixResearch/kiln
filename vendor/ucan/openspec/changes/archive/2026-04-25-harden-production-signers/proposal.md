## Why

The crate can issue and verify tokens, but signing is still tied to an in-memory Ed25519 seed type. That is acceptable for examples, but reusable crates need safe defaults: secret bytes must not leak through formatting, signing must not require exposing raw seed bytes, and the issue path should be able to use future KMS/HSM/remote signers without changing token semantics.

This is the highest-priority follow-up because every other workflow depends on safe key custody and signer boundaries.

## What Changes

- Redact or remove secret-key debug output and add tests that prove secrets are not formatted.
- Introduce a signer abstraction for token issuance so callers can provide in-memory, remote, or hardware-backed signers.
- Keep the current Ed25519 implementation as the first concrete signer and provide safe construction paths for test and production use.
- Keep verified signature semantics routed through `vendor/verified-logic`.

## Non-Goals

- Implement network KMS, HSM, or OS keychain integrations in this change.
- Add non-Ed25519 algorithms in this change.
- Move key generation or signing into `ucan-core`.

## Capabilities

### New Capabilities
- `library.production-signers.secret-redaction`: Secret material is protected from accidental formatting leaks.
- `library.production-signers.signer-abstraction`: Token issuance can use a signer interface instead of raw key access.
- `library.production-signers.ed25519-adapter`: The current Ed25519 key support remains available through the new interface.
- `library.production-signers.consumer-onboarding`: Examples and docs teach the signer boundary as the production-oriented path.

### Modified Capabilities
- `library.production-signers.fcis-boundary`: Signer I/O remains in the std shell and does not add signer/key-generation dependencies to `ucan-core`.

## Impact

- **Files**: `src/token.rs`, `src/verified.rs`, tests, examples, README/API docs.
- **APIs**: Adds signer traits/adapters and may de-emphasize direct `Ed25519SigningKey` issuance APIs.
- **Dependencies**: May add `zeroize` and an RNG crate if key generation is included.
- **Testing**: Needs positive issuance tests through the signer abstraction and negative/redaction tests for secret formatting and bad signer outputs.
