## Phase 1: Core and provider

- [x] [serial] Add `ReviewedHunk`, `HunkFact`, `Change.reviewed_hunks`, `Change.last_viewed`, `Facts.hunks`, `Command::MarkHunk`, file-level mark-all over hunk facts and carry-forward on upload in `kiln_core::change_review`, with positive and negative tests. r[kiln.review.hunks] r[kiln.review.hunks.scenario.carry_forward] r[kiln.review.hunks.scenario.replay]
- [x] [serial] Add `interdiff` and `revision_commits` to the provider port and the git adapter (in-memory rebase, change-id headers, unavailable reasons), with fixture tests. r[kiln.review.interdiff] r[kiln.review.interdiff.scenario.rebased] r[kiln.review.interdiff.scenario.unavailable] r[kiln.review.interdiff.scenario.narrowed] r[kiln.review.commit_range]

## Phase 2: Shell and console

- [x] [serial] Compute hunk digests in the review shell, pass hunk facts, add the mark-hunk action, interdiff/commit-range diff parameters with explicit fallback and per-principal `review_hunks`. r[kiln.review.hunks] r[kiln.review.interdiff] r[kiln.review.commit_range]
- [x] [serial] Console selector, route keys with back/forward, hunk toggles, viewed counts and new-since badges, commit-range picker, docs. r[kiln.review.interdiff.route] r[kiln.review.hunks]

## Phase 3: Verification

- [x] [serial] Run focused tests, node console tests and a disposable-instance browser check of selector, URL restore, mark/unmark and carry-forward; record evidence. r[kiln.review.interdiff] r[kiln.review.interdiff.route] r[kiln.review.commit_range] r[kiln.review.hunks]
