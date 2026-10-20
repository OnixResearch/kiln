## Context

`DecodedClaims.nbf` and `DecodedClaims.exp` are optional after serde parsing. Presence is verified before core construction, but their strict ordering (`nbf < exp`) is currently admitted only by ordinary Rust/core validation. This change moves that exact finite predicate into `verified-logic` while leaving runtime parsing and clock semantics unchanged.

## Goals

- Verify parsed temporal order for decoded `nbf` and `exp` values.
- Reject `nbf >= exp` through a verified primitive before constructing core claim input.
- Keep existing runtime temporal validity checks (`now` versus the time window) and clock tolerance behavior intact.

## Non-Goals

- Do not prove JSON/serde parser correctness, duplicate-key semantics, numeric range provenance, wall-clock freshness, clock source correctness, expiration-at-now policy, proof-reference decoding, proof-store/resolver/revocation truth or freshness, signing-input provenance, or cryptographic correctness.
- Do not change token issuance duration semantics or proof traversal behavior.

## Decisions

### Add a narrow parsed temporal-order bridge

**Choice:** Introduce a new vendored `ucan_temporal_order` primitive and UCAN bridge rather than overloading the parsed-presence predicate.

**Rationale:** Presence and ordering are adjacent but distinct admission facts. A separate bridge keeps the predicate exact (`not_before_seconds < expires_at_seconds`) and avoids broad claims about current-time validity.

## Validation

Run sibling verified-logic focused tests/Verus, sync vendor, run focused UCAN tests, then full UCAN gates and OpenSpec archive.
