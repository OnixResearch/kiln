# Design: Single-writer Radicle COB mutation

## Context

Kiln already owns the pure mutation flow: `CobEffectPlan`, fresh-observation preflight, plan admission, signer acquisition, preview, dispatch, and reconciliation with preserved unknown outcomes.

The missing piece is the concrete transport. It must sign and write collaborative-object updates against local Radicle storage using the cohort pinned in `publish-radicle-ci-status`.

## Goal and Success Contract

The goal is a versioned, preview-first collaboration mutation shell with single-writer authority.

Completion requires these results:

- a typed profile admits declared RIDs, declared object types, declared target identities, bounds, and timeouts;
- every mutation plan carries expected heads that the preflight re-reads immediately before signing;
- single-writer policy accepts only heads authored by the resolved Kiln signer or an empty object;
- the signer resolves through the same SecretSpec declaration mechanism as status publication;
- a missing or unresolved signer denies the plan before signing with no key leakage;
- ambiguous writes stay unknown until reconciliation; and
- native fixtures prove authorized writes plus every rejection class.

False completion includes ambient identity lookup, accepting foreign heads, merging concurrent updates, creating or closing patches, or fixture-only evidence.

## Decisions

### Decision: Reuse the pure mutation flow unchanged

**Choice:** The transport implements the existing `CobMutationTransport` port. Pure ordering stays exactly as published.

**Rationale:** The flow is tested and reviewed. Only provider calls are new.

### Decision: Declared targets only

**Choice:** The profile lists RIDs, admitted object types (`xyz.radicle.patch` first), and explicit target object identities. Plans outside all three lists fail before repository access.

**Rationale:** Collaboration writes can change review-visible state. Explicit targets make authority reviewable per deployment.

### Decision: Reload, compare, then sign

**Choice:** The shell reads current heads, releases that view, resolves the SecretSpec signer, and previews the rendered update. Dispatch then reloads and compares the exact head immediately before signing.

The Git reference write also uses an atomic expected-target comparison. Any mismatch, resolution failure, or preview rejection aborts before signing.

**Rationale:** This orders every denial before the first irreversible effect.

### Decision: Timeouts bound the storage view

**Choice:** Named deadlines bound preflight, signing, and write phases. A deadline after possible acceptance becomes unknown and requires reconciliation.

**Rationale:** The shell does not hold a storage view while it resolves secrets or previews payloads. Bounded phases keep outcomes honest without long-lived locks.

## Functional Core and Imperative Shell

Pure functions own: target admission, head comparison, single-writer decisions, plan rendering checks, and outcome classification.

The shell owns: SecretSpec resolution, keys, storage handles, transactions, clocks, and provider error capture.

## Validation

Positive fixtures cover an authorized first update, repeat of one operation identity, and reconciliation of a recorded write.

Negative fixtures cover moved heads, foreign authors, undeclared targets, undeclared types, missing signer, oversized payloads, timeouts, disconnects, and secret leakage.

## Risks and Trade-offs

- Single-writer rejects concurrent external updates instead of merging; operators accept visible rejections.
- `radicle-cob` transaction semantics can change; exact pins keep drift observable.
- Signing latency inside a storage view adds contention; named bounds limit it.

## Claim Boundary

Passing checks prove bounded local mutation under the pinned profile. They do not prove peer convergence, actor trust, remote visibility, merge correctness, or that any node serves the written state.
