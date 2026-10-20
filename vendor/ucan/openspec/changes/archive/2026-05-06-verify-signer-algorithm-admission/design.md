## Context

UCAN already routes decoded compact-token header `alg=EdDSA` and `typ=JWT` admission through `verified_logic::ucan_header::parsed_header_admission_is_valid`. Signer-based issuance separately accepts `TokenSigner::algorithm()` only when the returned string is `EdDSA`. That is a deterministic adapter fact with no runtime I/O or crypto dependency.

## Goals / Non-Goals

**Goals:**
- Reuse the existing verified supported-algorithm byte model for signer algorithm admission.
- Fail closed for any signer algorithm byte string other than exact `EdDSA`.
- Keep signer identity, public-key derivation, signing, and signature verification boundaries unchanged.

**Non-Goals:**
- No claim that a signer truthfully reports its algorithm.
- No Ed25519 cryptographic proof, RNG proof, key provenance proof, or signer I/O proof.
- No support for additional algorithms.

## Decisions

### 1. Add the helper to `ucan_header`

**Choice:** add `supported_algorithm_is_valid(&Vec<u8>) -> bool` next to the existing supported-algorithm spec and byte constructor.

**Rationale:** parsed token headers and signer algorithm admission share the exact `EdDSA` supported-algorithm value. Keeping both on `ucan_header` avoids a duplicate model in `ucan_signature`.

**Alternative:** use `parsed_header_admission_is_valid(algorithm, b"JWT")` from the UCAN shell. Rejected because signer admission has no token type fact; constructing a dummy type would obscure the exact seam.

## Risks / Trade-offs

**String-to-byte provenance** → UCAN still converts `&str` to bytes in the shell. The helper verifies only byte equality after that conversion.

**Overclaiming signer trust** → docs and specs explicitly keep signer truth/provenance and crypto outside this seam.
