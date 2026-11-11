## Phase 1: Core

- [x] [serial] Add bounded policy classes, per-rule approval modes and admission. r[kiln.review.class.policy]
- [x] [serial] Add ordered exact classification over current and renamed paths, with `logic` fallback. r[kiln.review.class.select]
- [x] [serial] Apply evidence-only mode to only the rule owner item while keeping evidence and ordinary authority. r[kiln.review.class.authority]
- [x] [serial] Add positive and negative tests for docs evidence-only, logic fallback, rename classification and policy bounds. r[kiln.review.class.select] r[kiln.review.class.authority]

## Phase 2: Contracts and verification

- [x] [serial] Add the Nickel class and approval contracts, update the positive fixture and document the fields. r[kiln.review.class.policy]
- [x] [serial] Run focused and full core/web-service tests, clippy, rustfmt, Nickel typecheck/export and Cairn validation. r[kiln.review.class.authority]
