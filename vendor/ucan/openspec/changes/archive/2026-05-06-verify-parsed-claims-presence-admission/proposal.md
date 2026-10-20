## Why

Decoded UCAN payload claims currently pass from serde-owned optional fields into `ucan-core` structural validation. The core validation is deterministic, but the shell lacks a narrow verified admission point for the parsed-presence facts that determine whether required claim fields are present before validation continues.

## What Changes

- Add a finite verified-logic predicate for shell-supplied parsed claims presence facts.
- Bridge the predicate into UCAN as parsed-claims admission before constructing `ClaimsInput`.
- Keep serde parsing, duplicate-key handling, field extraction correctness, string validation, capability/proof parsing, signing-input provenance, and cryptography outside the verified claim.

## Impact

- Files: `src/verified.rs`, `src/token.rs`, public exports, docs, bridge guard, vendored verified-logic.
- Verification: sibling verified-logic tests/Verus, UCAN cargo/Nix/OpenSpec gates.
