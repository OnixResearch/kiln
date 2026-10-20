## Context

`AuthorizationRequest::new` validates a requested resource/ability as a `CapabilityDocument`, then separately rejects wildcard segments with a shell-local `ability.split('/')` helper. `VerifiedCapability` already stores verified-logic parsed ability segments and is used for containment checks.

## Goals / Non-Goals

**Goals:** route concrete request wildcard rejection through `VerifiedCapability`; preserve all existing error shapes and validation behavior; add a guard against reintroducing local wildcard admission helpers.

**Non-Goals:** change UCAN ability grammar, add wildcard support to concrete requests, modify caveat semantics, or change `ucan-core`.

## Decisions

### VerifiedCapability owns concreteness

**Choice:** expose `VerifiedCapability::has_wildcard_segment` and `VerifiedCapability::is_concrete`.
**Rationale:** the wrapper already owns verified ability segments, so it is the narrowest bridge for checking whether any parsed segment is the wildcard segment.
**Alternative:** keep the shell-local split helper; rejected because it duplicates ability semantics outside the verified bridge.

### Preserve AuthorizationRequest errors

**Choice:** map any bridge construction error to the existing `TokenError::Capability` shape and keep the existing wildcard message.
**Rationale:** consumers should see no API/error-shape churn from this hardening slice.

## Risks / Trade-offs

**Extra clone** → `AuthorizationRequest` construction clones one small capability document for bridge admission. This is acceptable for a correctness boundary and can be optimized later if needed.

## Validation Plan

Run focused authorization/verified tests first, then the standard UCAN gates and strict OpenSpec validation.
