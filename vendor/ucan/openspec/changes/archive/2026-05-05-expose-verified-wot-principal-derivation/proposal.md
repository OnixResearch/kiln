## Why

The UCAN shell already exposes the composed Web-of-Trust UCAN authorizer, but callers must still prepare finite trusted-root and subject principal digests outside the public bridge surface. Vendored `verified-logic` already has an exact primitive for deriving a Web-of-Trust principal identifier from public key bytes. Exposing that primitive lets callers construct WOT bridge inputs without reimplementing digest derivation locally.

## What Changes

- Add a public data-only helper that derives a Web-of-Trust principal digest from caller-supplied public key bytes through vendored `verified_logic::web_of_trust_principal`.
- Re-export the helper and document that it only derives deterministic public principal bytes; it does not resolve identities, validate trust, fetch records, or perform authorization.
- Add focused tests, bridge guard coverage, and public API documentation.

## Capabilities

### New Capabilities
- `library.verified-wot-principal-derivation`: callers can derive WOT principal digests for finite WOT authorizer inputs through the vendored verified bridge.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, `scripts/check-ucan-bridge.sh`, `README.md`, `docs/public-api.md`, OpenSpec library spec.
- **APIs**: new public helper for Web-of-Trust principal digest derivation.
- **Dependencies**: no new dependencies.
- **Testing**: focused Rust tests, bridge guard, OpenSpec strict validation, full Cargo/clippy/no-std/Nix gates before archive.
