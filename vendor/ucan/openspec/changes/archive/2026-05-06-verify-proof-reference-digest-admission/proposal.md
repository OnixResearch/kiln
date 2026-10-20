# Change: Verify proof-reference digest admission

## Why

UCAN proof references are currently derived by hashing compact-token bytes with BLAKE3 in the shell and wrapping the resulting bytes directly in `ProofReference`. The BLAKE3 implementation remains a runtime-trusted cryptographic dependency, but the shell can still route the proof-reference admission boundary through vendored verified logic so the accepted reference shape is exact and auditable.

## What changes

- Add a vendored `verified-logic` UCAN proof-reference primitive that admits only fixed-width BLAKE3 digest bytes and exact digest/reference byte equality.
- Add a public `ucan` bridge helper for proof-reference digest admission.
- Route shell proof-reference construction after BLAKE3 hashing through that bridge.
- Document that this verifies fixed-width digest/reference admission only, not BLAKE3 correctness, token provenance, compact parsing, base64url, JSON, or cryptographic signature internals.

## Impact

- Tightens the proof-reference hash boundary without changing the public proof-store or revocation traits.
- Keeps `ucan-core` no-std and keeps hashing in the std shell.
