## Context

`DecodedClaims` stores serde-parsed UCAN payload fields as `Option` values. `validate_decoded_token` converts those fields into `ucan_core::token::ClaimsInput`, where missing required fields reject. A verified bridge can admit only the boolean presence facts that the shell derives from those `Option` values.

## Goals

- Verify a tiny predicate: issuer, audience, attenuations, expiration, and not-before parsed-presence facts must all be true.
- Route `validate_decoded_token` through the predicate before core claims construction.
- Expose a public bridge helper for callers/tests that want the same finite admission.

## Non-Goals

- Do not verify serde, JSON grammar, duplicate-key semantics, UTF-8/string normalization, DID/capability/proof parsing, capability non-emptiness, proof-reference shape, temporal ordering, signing-input canonicalization, signatures, hashes, resolver/proof-store/revocation behavior, or token provenance.

## Validation

Run sibling focused Rust/Verus checks, sync vendor, then run UCAN focused tests, bridge guard, OpenSpec validation, full cargo checks, no-std guard, diff check, and Nix flake check.
