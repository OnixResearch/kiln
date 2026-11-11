# Verification evidence

## Result

The shared workflow configuration now maps a custom effect class to the
provider-neutral custom class. The pinned Lattice profile admits an ordered
`review` node kind and the `Custom` effect class alongside existing build and
test capabilities. The web service contract and Patchgate profile fixture admit
a review/custom node.

## Evidence

- All 20 Lattice adapter library tests pass, including a review/custom node
  lowering case and the existing profile/transport fixtures.
- GitHub and Gitea custom effect-class mapping tests pass.
- All 166 web-service library tests pass (1 ignored).
- All 81 engine integration tests pass, including a review-node run that accepts
  a `kiln.patchgate.review.v1` receipt as a run-bound review report.
- Nickel typecheck and export pass for the Lattice, service and Patchgate
  fixtures; every negative Lattice profile fixture remains rejected.
- Formatting passes for all changed crates.
- `cairn validate --root .` reports zero issues.

## Non-claims

Node admission does not prove the Lattice runtime executes custom review nodes.
The review node remains observation-only and grants no acceptance, merge, gate
or release authority. This change does not prove a live Patchgate/Pijul review
or review quality. The worker still requires an admitted step and an accessible
Pijul target.
