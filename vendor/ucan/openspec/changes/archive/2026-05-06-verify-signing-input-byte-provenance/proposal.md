## Why

Compact framing and envelope admission already verify exact `header.payload.signature` and signing-input byte equality for already split parts, but the live signature verification path should explicitly preserve the admitted signing input bytes into the bytes passed to signature verification.

## What Changes

- Strengthen or add a vendored signing-input provenance primitive over admitted compact-token header/payload bytes and the signature-verification input bytes.
- Route live signature verification input construction through a UCAN bridge before Ed25519 verification.
- Keep Ed25519 correctness, key resolution, JSON canonicalization, parser correctness, and cryptographic strength outside the verified claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, signature verification path in `src/token.rs`, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a signing-input byte provenance bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
