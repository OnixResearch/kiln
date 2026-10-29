## Phase 1: Implementation

- [x] [parallel] Implement pure changes, revisions, threads, drafts, reviewer roles, votes, checklists, stacks, search, notifications and saved views. r[kiln.review.workflow]
- [x] [parallel] Implement versioned provider operations and exact-target local collaboration/diff adapters with conformance cases. r[kiln.review.provider]
- [x] [parallel] Implement bounded durable review API, principal isolation, request replay and effect reconciliation. r[kiln.review.authority]
- [x] [parallel] Implement integrated change pages, diff interactions, review/author actions, discovery and attention. r[kiln.review.surface]
- [x] [serial] Integrate server configuration, canonical CI facts, typed policy and operator documentation. r[kiln.review.authority]

## Phase 2: Verification

- [x] [serial] Exercise complete workflow, negative cases, restart, authenticated desktop/mobile UI and required repository checks; retain bounded evidence. r[kiln.review.workflow]

Runtime, workspace tests, Clippy, formatting and x86_64 Nix flake checks passed.
`evidence/` retains actual runtime observations, screenshots and qualification
holds. Accepted-spec traceability and release-readiness remain failing while
this and other changes are active; the explicit active-development profile
resolves the review requirements but retains three unrelated dangling IDs.
No specification sync, archive, release approval or deployment is implied.

