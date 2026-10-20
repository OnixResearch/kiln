# Design

## Verified primitive

Extend `verified_logic::ucan_signature` with executable helpers that accept an observed `usize` length and return whether it matches the fixed Ed25519 widths:

- `secret_key_len_is_valid(actual)`
- `public_key_len_is_valid(actual)`
- `signature_len_is_valid(actual)`

Each helper has a small Verus contract tying the runtime result to the existing fixed-width constants and byte-validity specs. Existing checked constructors remain available for callers that already own a `Vec<u8>`.

## UCAN bridge

UCAN keeps its public error mapping because the shell must report `VerifiedPrimitiveError::{InvalidSecretKeyLength,InvalidPublicKeyLength,InvalidSignatureLength}` with the observed length. The local validators become thin adapters:

1. read `bytes.len()`,
2. call the vendored helper,
3. return `Ok(())` or the existing structured error.

## Boundary

This proves only deterministic byte-length admission before typed signature/key struct construction. It does not prove cryptographic correctness, signing-key provenance, key derivation, signature verification, DID semantics, token canonicalization, parser behavior, resolver/proof-store truth, clocks, revocation, or authorization policy.
