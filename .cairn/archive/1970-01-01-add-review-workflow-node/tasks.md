## Phase 1: Capability admission

- [x] [serial] Add `Custom` to the shared workflow configuration effect classes and map it to the core custom class. r[kiln.lattice.review_node]
- [x] [serial] Add the ordered `review` node kind and `Custom` effect class to the pinned Lattice profile and Nickel contract. r[kiln.lattice.review_node]
- [x] [serial] Allow `effect_class = "custom"` in the web service workflow contract. r[kiln.lattice.review_node]

## Phase 2: Verification

- [x] [serial] Add review/custom lowering and effect-class mapping tests plus a service review-node fixture. r[kiln.lattice.review_node]
- [x] [serial] Exercise a review-node run that retains a Patchgate receipt as run-bound review evidence. r[kiln.lattice.review_node]
- [x] [serial] Run focused crate tests, Nickel checks, formatting and lifecycle validation. r[kiln.lattice.review_node]
