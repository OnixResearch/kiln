## Why

UCAN now verifies required-claim presence and several parsed list/flow facts before constructing core claims, but decoded issuer and audience DID value admission still falls directly through ordinary Rust/core trimming checks. A narrow parsed-DID admission seam can reduce this trusted shell surface without claiming DID-method resolution or semantic identity correctness.

## What Changes

- Add a vendored verified-logic primitive that admits shell-supplied parsed issuer/audience DID shape facts: present, non-empty, and already trimmed.
- Route decoded `iss` and `aud` facts through a UCAN verified bridge before building `IssuerDid`, `AudienceDid`, or core claims.
- Document that JSON parsing, UTF-8/string provenance, DID method semantics, key resolution, canonicalization, and identity truth remain outside this seam.

## Impact

- Affected specs: `openspec/specs/library/spec.md`.
- Affected code: `src/verified.rs`, `src/token.rs`, `src/lib.rs`, `scripts/check-ucan-bridge.sh`, vendored `verified-logic`.
- Verification: sibling Rust/Verus checks, UCAN cargo/Nix/OpenSpec gates, archive, commit, and push.
