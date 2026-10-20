## Context

`verify_compact_token` returns a `VerifiedToken` with effective delegation data. Application crates still need to turn that result into an allow/deny decision for one requested resource and ability. That matching must use the same verified semantics as delegation attenuation.

## Goals / Non-Goals

**Goals:** provide a first-class authorization API, return structured decisions, reuse verified capability containment, and keep request matching deterministic.

**Non-Goals:** caveat evaluation, revocation, external policy stores, or invocation-specific request facts. Those are handled by later changes.

## Decisions

### 1. Add explicit authorization request and decision types

**Choice:** Model requested resource/ability as an `AuthorizationRequest` and return an `AuthorizationDecision` or `AuthorizationResult` with allow/deny detail.

**Rationale:** A named request makes the API self-documenting and prevents downstream crates from duplicating matching logic.

**Alternative:** Add a boolean `contains` helper only. Rejected because it loses denial detail and malformed-input separation.

**Implementation:** Validate request resource/ability through existing core capability rules before matching.

### 2. Match only against effective delegation

**Choice:** Authorization uses `VerifiedToken::effective_delegation().capabilities()` rather than direct token capabilities.

**Rationale:** Direct child claims may be narrower than parents, but effective delegation is the already-verified authority boundary. Consumers should not accidentally authorize from unverified or direct-only data.

**Alternative:** Let callers choose direct or effective claims. Rejected for the primary API because it is easy to misuse.

**Implementation:** Expose direct-claim inspection separately; make authorization methods use effective data by default.

### 3. Use verified-logic-backed containment

**Choice:** Reuse `VerifiedCapability::contains` or a small wrapper around it for matching.

**Rationale:** This keeps wildcard and resource-scope semantics aligned with proof-chain attenuation.

**Alternative:** Implement string prefix or ad-hoc wildcard matching. Rejected because it would drift from verified semantics.

## Risks / Trade-offs

**Decision type can grow with caveats later** → Use an enum or struct shape that can add caveat denial reasons without breaking core matching.

**Too many convenience methods** → Provide one canonical method first, then add aliases only after examples prove the need.
