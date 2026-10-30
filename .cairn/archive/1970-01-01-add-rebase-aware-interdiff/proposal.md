## Why

Every new patch set forces a full re-review (issue #5). Viewed markers are per file and bound to one revision, and `Upload` clears all of them (`current.reviewed.clear()`). "Compare against" an earlier revision diffs the two revision commits directly, so after a rebase the reviewer re-reads everything that moved on the target. Re-review cost per patch set is the part Kiln can remove without weakening any authority.

## What Changes

- **Provider interdiff:** `ChangeReviewPort::interdiff` compares two revisions with base movement removed: the old change is rebased in memory onto the new base (three-way tree merge, no ref or object written to disk) and the result is diffed against the new commit. A conflicted rebase, a missing base or commit, or a provider without the capability returns an explicit `unavailable` reason. The shell then falls back to the existing exact commit comparison with a notice; an approximation is never presented as exact.
- **Stack narrowing:** an interdiff may be narrowed to one Jujutsu change ID. Each revision's commit range is listed with each commit's `change-id` header (validated with `kiln_core::source::valid_jujutsu_change_id`). Missing change IDs, an absent or duplicated change ID on either side returns `unavailable` with the exact fallback.
- **Commit-range view:** the commits of a multi-commit revision are listed and any `from..to` range inside the revision (the base included as a start) can be compared exactly.
- **Hunk markers:** `ReviewedHunk { actor, revision, file, hunk_digest }`; the shell computes `hunk_digest` as BLAKE3 over the normalized hunk (path, removed and added lines, no line numbers) and passes hunk facts to the pure core. A per-hunk toggle and the file-level "mark all" both write hunk markers. On upload, a principal's markers carry forward only for hunks whose digest is present unchanged in the new revision. Each file shows "N of M hunks viewed" and which hunks are new since the principal's last viewed revision.
- **Console:** the comparison selector offers "Changes since revision N"; `review_interdiff=<from>..<to>` (Kiln revision ids), optional `review_interdiff_change=<change-id>` and `review_commits=<from>..<to>` are URL-restored, with back/forward navigation.
- **Compatibility:** existing `ReviewedFile` markers keep their meaning on their original revision only; state documents without hunk markers serialize byte-identically.

## Impact

- **Consumers:** reviewers of revised changes; jj users with stacked commits.
- **Owners:** Kiln maintainers own `kiln_core::change_review` and the provider port; Kiln web-service maintainers own the git adapter, review shell and console.
- **Affected areas:** `crates/kiln-provider-api`, `crates/kiln-adapter-git`, `crates/kiln-core` (`change_review`), `crates/kiln-web-service` (code review shell, `web.js` route keys, `code-review.js/.css`), `docs/operator.md`.

## Out of Scope

- Approval, ownership, submit, CI or queue admission: markers stay private preferences and never count as approval.
- Retargeting historical drafts or threads: anchors keep exact revision/base/side/line coordinates; interdiff old-side lines (an in-memory rebased tree) cannot be commented on.
- Keyboard toggles for hunks (#9) and noise pre-viewing (#7) beyond the published hook points.
- Mutating Jujutsu or Git state; the in-memory rebase writes nothing to the repository.
