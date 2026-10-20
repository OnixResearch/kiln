## Context

UCAN exposes many public verified helpers from `src/lib.rs`, routes a subset through live compact-token verification paths, and vendors additional verified-logic UCAN modules. Some helpers are intentionally advanced/public-only because they consume caller-supplied facts; some vendored modules are proof/spec-only and have no executable bridge suitable for the UCAN shell.

## Goals

- Make bridge coverage auditable from code, docs, specs, and guard scripts.
- Separate live verifier guarantees from opt-in public helper guarantees.
- Avoid overclaiming on vendored modules that model CID/principal-link/hash behavior without executable UCAN shell bridge functions.
- Keep cleanup small enough to land in one sweep.

## Non-Goals

- Proving runtime base64url, serde JSON, BLAKE3, Ed25519, resolver, proof-store, revocation backend, DID method, CID, principal-link, wall-clock, or arbitrary caveat-policy correctness.
- Replacing caller-injected backends or policies with defaults.
- Adding new verified-logic primitive families as part of this cleanup.

## Decisions

### 1. Coverage classes

Each verified bridge or vendored UCAN module will be classified as one of:

- `live-path`: called by the verifier/token construction path before accepting a live token fact.
- `public-helper`: exposed for callers with already-supplied finite facts, but not a live verifier guarantee.
- `documented-boundary`: intentionally not routed or exposed because no exact executable bridge exists or routing would overclaim.
- `candidate`: exact executable bridge exists and the shell already supplies matching finite facts, so implementation may route it in this sweep.

### 2. Exactness before routing

The cleanup may only route a bridge when the runtime shell facts are the exact inputs required by the vendored predicate. If the vendored module uses spec-only/proof-only models, uninterpreted digest specs, or a different hash/identity model, the result remains `documented-boundary`.

### 3. Guards and docs stay synchronized

The bridge guard should fail when a required public/live bridge name disappears from expected documentation or live path routing. The boundary doc should name live-path and public-helper coverage separately enough that future audits do not infer internal verifier coverage from helper exports.

## Risks

- Overclaim risk: mitigated by keeping spec-only vendored modules as documented boundaries unless executable bridge exactness is shown.
- Scope creep: mitigated by treating new primitive work as a follow-up OpenSpec, not part of this sweep.
- Stale docs risk: mitigated by updating guard expectations alongside docs/specs.
