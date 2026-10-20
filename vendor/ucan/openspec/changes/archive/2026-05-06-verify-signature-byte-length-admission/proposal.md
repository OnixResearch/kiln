# Verify signature byte-length admission

## Summary

Route UCAN shell Ed25519 secret-key, public-key, and signature byte-length admission through vendored `verified_logic::ucan_signature` instead of local adapter comparisons.

## Motivation

`src/verified.rs` currently checks byte slice lengths locally before constructing `verified_logic::ucan_signature` structs. The facts are deterministic, finite, and already part of the sibling signature model, so the shell should consume verified admission helpers rather than own parallel length predicates.

## Scope

- Add or expose executable byte-length admission helpers in sibling `verified-logic::ucan_signature`.
- Sync the vendored snapshot into UCAN.
- Route `validate_secret_key_length`, `validate_public_key_length`, and `validate_signature_length` through the vendored helpers.
- Update docs and bridge guards to prevent reintroducing local signature length comparisons.

## Non-Goals

- Proving Ed25519 cryptography, dalek internals, signing key provenance, DID resolution, parser correctness, or token semantic authorization.
- Moving secret storage, signer abstractions, or runtime crypto into `ucan-core`.
- Adding cleartext display/debug/serde for secret material.
