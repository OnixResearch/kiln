## Context

The aggregate `web_of_trust_ucan_authorization` bridge already composes vendored WOT principal, attestation, revocation, caveat, and authorization semantics over caller-supplied finite public data. The audit found additional exact vendored public helpers in `web_of_trust_attestation`, `web_of_trust_revocation`, and `web_of_trust_policy` that are not currently exposed as standalone `ucan` APIs.

## Goals / Non-Goals

**Goals:** expose narrow, deterministic helpers for canonical attestation/revocation payload bytes and semantic evidence predicates when they are exact wrappers over vendored verified-logic.

**Non-Goals:** no DID resolution, proof fetching, private-key handling, signature creation, revocation registry freshness, persistence, replay protection, networking, ambient clock reads, or replacement for shell-owned UCAN verification/caveat policy.

## Decisions

### 1. Keep helpers finite and data-only

**Choice:** Each helper accepts already-materialized public WOT evidence fields and explicit time/signature-valid facts required by the vendored primitive.

**Rationale:** This preserves the current caller-injected boundary and avoids hidden runtime authority.

**Alternative:** Add a WOT evidence verifier that fetches records or verifies signatures internally. Rejected because it would add resolver/backend/crypto scope beyond the exact vendored primitive.

### 2. Prefer semantic evidence predicates over payload-only APIs when scope must be reduced

**Choice:** If the full helper set is too broad, implement revocation match/effective-at and attestation active/signature-valid predicates before canonical payload-byte construction.

**Rationale:** Semantic booleans are safer and easier to document than payload construction APIs that callers might mistake for signing support.

## Risks / Trade-offs

**API sprawl** → Keep names explicit and grouped under verified bridge documentation.

**Misuse as complete WOT verification** → Documentation and tests MUST assert the helpers do not resolve identities, fetch evidence, or replace aggregate authorization.
