## Context

`DecodedClaims.prf` is optional after serde parsing. Missing `prf` is rejected by `ucan-core`, but that rejection currently occurs after the shell's parsed-presence bridge. The exact closeable fact is small: the serde/shell observed whether the field was present.

## Goals

- Verify finite parsed proof-list presence as part of parsed claims admission.
- Reject missing `prf` through the verified primitive before constructing core claim input.
- Keep empty proof lists valid when present.

## Non-Goals

- Do not prove JSON/serde parser correctness, duplicate-key semantics, field extraction truth, proof-reference string decoding, base64url decoding, BLAKE3 proof-reference derivation, proof-store/resolver/revocation truth or freshness, signing-input provenance, or cryptographic correctness.
- Do not change proof traversal behavior or add backends.

## Decisions

### Extend the existing parsed-claims presence primitive

**Choice:** Add `proofs_present` to the existing `VerifiedParsedClaimsPresence` / vendored `ParsedClaimsPresence` predicate rather than creating a second one-field bridge.

**Rationale:** `prf` is the remaining required parsed claim in the same admission class; one predicate avoids split-brain presence checks while preserving exact scope.

## Validation

Run sibling verified-logic focused tests/Verus, sync vendor, run focused UCAN tests, then full UCAN gates and OpenSpec archive.
