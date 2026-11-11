# Lattice Workflow Adapter Specification

## ADDED Requirements

### Requirement: Reviewed review node admission

r[kiln.lattice.review_node] The pinned Lattice capability profile MUST retain
ordered build and test capabilities and MAY additionally admit a `review` node
kind with the `Custom` effect class. The service workflow configuration MUST
map a configured custom effect class to the provider-neutral custom class.
Admission MUST NOT imply review acceptance, merge, gate, release, or source
mutation authority.

#### Scenario: Review node lowers

- GIVEN a reviewed workflow plan containing one `review` node with the custom
  effect class
- WHEN Lattice lowering and contract admission run
- THEN the request MUST retain the review node kind and custom effect class and
  MUST remain an observation-only workflow.

#### Scenario: Review receipt is retained

- GIVEN a review-node run and a Patchgate receipt uploaded as a review report
- WHEN the run reports are read
- THEN the recorded review must retain the receipt document and claim boundary
  without granting review acceptance.

#### Scenario: Unsupported capability remains rejected

- GIVEN a profile or plan with an unadmitted node kind, an unadmitted effect
  class, or mismatched capability ordering
- WHEN admission runs
- THEN it MUST reject before transport rather than fall back to a broader
  profile.
