## Context

The existing WOT UCAN authorizer bridge accepts trusted roots, attestation issuer/subject identifiers, revocation issuer/family identifiers, and subject identifiers as fixed-width principal digest bytes. Internal tests derive those bytes with `verified_logic::web_of_trust_principal::principal_id_from_bytes`, but the public UCAN shell does not expose that derivation helper.

## Goals / Non-Goals

**Goals:**
- Expose a deterministic public-key-bytes-to-principal-digest helper through the `ucan` verified bridge module.
- Keep the helper data-only and explicit about identity/trust non-claims.
- Reuse vendored `verified_logic::web_of_trust_principal` rather than adding shell-local digest logic.

**Non-Goals:**
- No DID resolution, key fetching, attestation lookup, trust-chain validation, authorization decisions, revocation freshness, networking, persistence, replay protection, signing, or ambient clock reads.
- No replacement for `web_of_trust_ucan_authorization` or production UCAN verification.

## Decisions

### Public helper shape

**Choice:** add `web_of_trust_principal_digest(public_key_bytes: &[u8]) -> Vec<u8>`.

**Rationale:** the vendored primitive accepts byte input and returns a fixed-width `PrincipalId`. A plain digest byte vector matches existing WOT bridge input fields and avoids introducing a second public wrapper type that callers would immediately unwrap.

**Alternative:** expose `VerifiedWebOfTrustPrincipal`. Rejected for this slice because the existing WOT authorizer API already uses digest bytes and validates width at conversion boundaries.

### Boundary

**Choice:** document the helper as deterministic derivation only.

**Rationale:** `principal_id_from_bytes` supplies digest identity bytes; it does not prove identity trust, resolve DIDs, fetch WOT records, or authorize a UCAN.

## Risks / Trade-offs

**Hash semantics overclaim** → Avoid claims about trust/security beyond deterministic public principal derivation. Keep authorization/trust decisions in the composed authorizer helper.

**API confusion with raw digest validation** → Tests cover fixed digest length and deterministic equality; docs direct callers to the composed WOT authorizer for trust checks.
