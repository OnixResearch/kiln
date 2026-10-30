## Context

- `kiln_core::change_review` is `no_std` + alloc and never hashes. `ReviewedFile { actor, revision, file }` markers are written by `Command::MarkReviewed` and cleared by `Command::Upload`. Shell facts (`Facts`) carry trusted observations such as anchors and changed files.
- `kiln-web-service` persists `State` as one SQLite JSON document; loading and committing it is the "replay" of review journals. `populate_changed_files` already runs the exact base comparison for the current or uploaded revision before every command.
- `LocalGitReviewProvider::compare` (kiln-adapter-git) produces `CommitComparison` with exact blob line coordinates under `DiffLimits`.
- The console comparison selector offers the selected revision's base or an earlier revision commit; `web.js` `readRoute` whitelists `review_*` query keys and `navigate` pushes history entries.

## Goals and Success Contract

A reviewer comparing revision N to revision M after a rebase sees only the author's delta, or an explicit unavailable notice with the exact commit comparison. Viewed state survives upload exactly for hunks whose normalized content is unchanged. Any interdiff, narrowed interdiff or commit-range view is restorable from its URL.

False completion includes: an interdiff shown as exact after a conflicted rebase; a marker carried for an edited or moved-with-edit hunk; old file markers changing bytes or meaning; a marker counted as approval; interdiff old-side lines accepted as comment anchors; a narrowed interdiff computed without change IDs.

## Decisions

### Decision: Interdiff is an in-memory rebase followed by an exact diff

**Choice:** `interdiff(old_base, old_commit, new_base, new_commit)` merges trees `ancestor=old_base, ours=new_base, theirs=old_commit` into an in-memory index, with an in-memory object backend (mempack) at the highest priority so merged blobs never reach the repository. A conflicted index returns `Unavailable { reason: conflicted_rebase }`; an unknown commit returns `missing_commit`; a provider lacking `Compare` returns `unsupported`. Otherwise the diff is `rebased-tree → new_commit` with the same bounds, whitespace mode and rename detection as `compare`. New-side coordinates are exact `new_commit` blob lines. The default trait method returns `unsupported` so other providers fail closed.

### Decision: Explicit fallback in the shell

**Choice:** the diff endpoint accepts `interdiff=<from revision id>` (with `revision=<to>`) and optional `change=<change-id>`. The response carries `interdiff: { status: "exact" | "unavailable", reason, from, to, change, fallback }`. When unavailable, the comparison is the exact commit comparison `from.commit → to.commit` and `fallback: "exact_commit_comparison"`; the console shows a notice naming the reason.

### Decision: Stack narrowing by change-id header

**Choice:** `revision_commits(base, head, limit)` lists the first-parent commits of `base..head` oldest first, each with `change_id` read from the git `change-id` commit header and admitted by `kiln_core::source::valid_jujutsu_change_id`. Narrowing to change X requires every commit on both sides to carry a valid change ID and X to occur exactly once on each side; the narrowed interdiff is then `interdiff(parent(old X), old X, parent(new X), new X)`. Otherwise `unavailable` with `no_change_ids`, `change_missing` or `change_ambiguous`. When the JjIntake shell observation for a revision exists it is a provider fact like the header; the header is the single read path here.

### Decision: Commit-range view

**Choice:** the diff endpoint accepts `commits=<from>..<to>` with full hex commits. `from` must be the revision base or a listed commit, `to` a listed commit after `from`. The comparison is the exact provider comparison of the two commits. Commenting is enabled only when `to` is the revision commit (new-side anchors on the base comparison); otherwise the console disables commenting with a note.

### Decision: Hunk digests are shell facts; markers live in core

**Choice:** the shell computes `hunk_digest = BLAKE3(JSON ["kiln.review.hunk.v1", path, [[kind, text]…]])` for each hunk of the revision's exact base comparison (`kind` is `-`, `+` or `\` for removed, added and end-of-file markers; context lines and line numbers excluded). A file without textual hunks (pure rename, mode change, empty file) gets one whole-file digest `["kiln.review.file.v1", path, old_path, old_blob, new_blob]`. `Facts.hunks: Option<Vec<HunkFact { file, digest }>>` (serde default, omitted when absent) carries them.

`Change.reviewed_hunks: Vec<ReviewedHunk>` and `Change.last_viewed: BTreeMap<actor, revision>` are serde-default and omitted when empty, so existing state documents re-serialize byte-identically. `MarkReviewed { file, reviewed }` becomes the file-level convenience: it writes (or removes) markers for every hunk of the file in `facts.hunks`, and unmarking also removes a legacy `ReviewedFile` row for that file and revision. `MarkHunk { file, hunk_digest, reviewed }` toggles one hunk; the digest must be a current fact. Missing facts fail closed with `hunks_unavailable`.

### Decision: Carry-forward on upload

**Choice:** `Upload` keeps, rebound to the new revision, exactly the markers on the previous revision whose `(file, digest)` occurs in the new revision's `facts.hunks`; all others and all legacy file markers are dropped (legacy markers keep their meaning on their original revision only). `last_viewed` is unchanged by upload and records each actor's latest revision with a positive mark.

### Decision: Viewed progress and "new since" in the diff response

**Choice:** the diff response includes `review_hunks` for the selected revision and the requesting principal only: per file `total`, `viewed`, `new_since_viewed`, the digest list, `last_viewed` (id and number) and, when the displayed comparison is the exact base comparison, per displayed hunk `{digest, viewed, new_since_viewed}`. "New since" is computed against the hunk digests of the principal's last viewed revision.

### Decision: Console surface and route

**Choice:** the selector adds "Changes since revision N" options (value `interdiff:<id>`). Route keys `review_interdiff`, `review_interdiff_change` and `review_commits` are validated in `web.js` `readRoute`; selecting writes them through `navigate`, so back/forward restore the view. File rows show "N of M hunks viewed"; hunk header rows carry a toggle button (`.cr-hunk-mark`, `data-hunk-digest`, `aria-pressed`) exposed for #9 as `KilnCodeReview.hunks.toggle(digest)` and `KilnCodeReview.hunks.list()`.

## Risks / Trade-offs

- A git hunk that merges with a neighbouring edit changes digest, so its markers drop; this is conservative.
- A renamed file changes every digest because the path is part of the normalized hunk.
- Interdiff costs one in-memory three-way tree merge per request, bounded by the same diff limits.
- Narrowing reads only the `change-id` header; repositories pushed without it report `no_change_ids`.

## Verification Strategy

- **Core tests:** mark all writes hunk markers, per-hunk toggle, unknown digest and missing facts rejected, carry-forward keeps unchanged hunks and drops edited, moved-with-edit and new hunks, legacy file markers round-trip byte-identically and are dropped on upload, markers never affect the checklist.
- **Adapter tests:** rebased fixture interdiff shows only the author's delta while plain comparison still shows target movement; conflicted rebase and missing base return unavailable; narrowed interdiff with change-id headers; stack without headers unavailable; commit listing.
- **Web-service tests:** digests ignore line numbers and context; diff endpoint fallback notice; replay of an old state document is byte-identical.
- **Browser:** disposable instance; selector, interdiff URL restore with back/forward, mark/unmark and carry-forward after upload.
