## Phase 1: Core policy and checklist

- [x] [serial] Add bounded interface/project ownership fields, admission and exact or literal-subset path matching. r[kiln.review.interface.policy]
- [x] [serial] Add exact dependent CI and dependent-owner checklist items. r[kiln.review.interface.ci] r[kiln.review.interface.ack]
- [x] [serial] Extend automatic reviewers with dependent owners. r[kiln.review.interface.ack]
- [x] [serial] Add positive and negative core tests for interface CI, acknowledgement, non-interface changes and admission. r[kiln.review.interface.policy] r[kiln.review.interface.ci] r[kiln.review.interface.ack]

## Phase 2: Contracts and verification

- [x] [serial] Add explicit policy-declared dependents, the Nickel ownership contract and operator docs. r[kiln.review.interface.policy] r[kiln.review.interface.boundary]
- [x] [serial] Reject interface/dependent project links outside the configured service project set. r[kiln.review.interface.policy]
- [x] [serial] Run focused and full core tests, clippy, rustfmt, Nickel and Cairn validation. r[kiln.review.interface.boundary]
