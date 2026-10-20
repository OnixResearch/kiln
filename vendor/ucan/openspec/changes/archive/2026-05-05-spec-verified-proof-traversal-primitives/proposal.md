## Why

The UCAN shell already documents proof traversal as the largest remaining high-ROI unverified surface. Existing bridge helpers cover individual proof links, depth budgets, capability containment, invocation authorization, and revocation-list membership, but the shell still owns traversal orchestration: fetching proofs, tracking visited references, enforcing budgets, folding effective delegation, and mapping fail-closed errors.

A new verified-logic primitive family is needed before the UCAN shell can route more traversal decisions through exact vendored checks without pretending that resolver, parsing, storage, crypto, caveat-policy, or backend behavior is formally verified.

## What Changes

- Define the desired finite proof traversal primitive family as data-only verified-logic targets.
- Model shell-produced traversal transcripts as the bridge boundary between impure resolution and pure verification.
- Require primitives for graph well-formedness, bounded traversal, effective delegation folding, chain authorization, revocation-aware denial, and transcript consistency.
- Preserve caller-injected shell boundaries and fail-closed backend behavior.

## Capabilities

### New Capabilities
- `library.verified-proof-traversal-primitives`: specifies exact future primitive contracts needed to verify more of UCAN proof traversal.

### Modified Capabilities
- `library.verified-proof-traversal-model`: refines the existing boundary model with concrete primitive families.

## Impact

- **Files**: OpenSpec artifacts only in this change.
- **APIs**: No public Rust API changes yet; future implementation may add vendored verified-logic types and bridge helpers.
- **Dependencies**: No dependency changes in this spec-only slice.
- **Testing**: Validate the OpenSpec package now; future implementation must add verified-logic proof evidence, UCAN bridge guards, and traversal regression tests.
