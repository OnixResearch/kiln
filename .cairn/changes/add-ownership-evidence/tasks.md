## Phase 1: Core

- [x] [serial] Add bounded `required_evidence` policy types, closed `project_ci` kind and policy admission. r[kiln.review.evidence.policy]
- [x] [serial] Add exact project/revision/scope CI evidence checklist items without changing owner approval. r[kiln.review.evidence.project_ci]
- [x] [serial] Add positive and negative tests for satisfied, stale, wrong-project, non-clean, partial and scoped evidence, and bounded/unique policy admission. r[kiln.review.evidence.project_ci] r[kiln.review.evidence.policy]

## Phase 2: Contracts and verification

- [x] [serial] Add the optional Nickel ownership evidence contract and document the policy field. r[kiln.review.evidence.policy]
- [x] [serial] Run focused and full core/web-service tests, clippy, rustfmt, Nickel typecheck/format and Cairn validation. r[kiln.review.evidence.project_ci]
