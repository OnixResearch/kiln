## Context

Match admitted controls to the real Lattice handler.

## Decision

Cut over the maintained profile to cancel-only without changing wire schema strings or vendored sources. Preserve exact contract provenance. Test cancellation plus unsupported-control rejection with a transport call counter, and migrate every consumer fixture.

## Verification

Run focused adapter and consumer tests and Nickel positive/negative fixtures. Exercise the changed boundary without external effects; record exact inputs and observed outcomes.

## Risks and non-claims

Contract compatibility is not runtime correctness. No deployment, publication, release eligibility or imported license change is established.
