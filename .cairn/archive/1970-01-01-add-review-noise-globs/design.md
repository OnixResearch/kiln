## Context

The code-review profile is `crates/kiln-web-service/src/code_review.rs` `Profile`, authored through `config/web-code-review.ncl` and admitted by `Profile::admit` (called by `kiln-web-service check` and `serve`). Ownership rules live in `kiln_core::change_review::Policy.ownership` and use `kiln_core::paths::{valid_pattern, matches}`. `Profile::binding()` is the store binding of the SQLite journal (a mismatch refuses the database), and `Profile::effective_policy()` hashes the policy into `policy.id`, which queue entries and submit candidates bind. The console (`code-review.js`) renders one exact provider comparison: a changed-file list plus the selected file's diff. Issue #5 (agent Interdiff) is concurrently replacing per-file viewed markers with per-hunk markers.

## Goals and Success Contract

Given declared noise rules in an admitted profile, every displayed file whose new or old path matches a rule is shown collapsed under a visible "N noise files hidden" group with the rule IDs, is pre-marked viewed for every principal, and expands with one click. The submit checklist, ownership, approvals, blocking threads, CI, queue admission and journals are byte-for-byte unaffected by the rules. Invalid or unbounded rules never reach a running service.

False completion includes: a noise rule that removes an ownership checklist item; a stored "viewed" marker written on behalf of a principal; a rule change that silently changes the store binding or policy identity (bricking journals or queue entries); patterns accepted by Nickel that Rust rejects or vice versa; content heuristics.

## Decisions

### Decision: Profile shape and limits

**Choice:** optional top-level `review: { noise: [{ id, pattern, reason }] }` (`Option<ReviewDisplay>`, serde default, omitted when absent so old profiles load unchanged). Named limits: `MAX_NOISE_RULES = 64`, `MAX_NOISE_PATTERN_BYTES = 256`, `MAX_NOISE_ID_BYTES = 128`, `MAX_NOISE_REASON_BYTES = 256`. Ids are unique, non-blank, without whitespace or control characters; reasons are non-blank text without control characters; patterns pass `kiln_core::paths::valid_pattern` (the ownership validator) within the byte bound. The Nickel contract mirrors the same predicate (`review | c.Nullable Review | default = null`).

### Decision: Display identity, not store or policy identity

**Choice:** `noise_identity = BLAKE3(JSON {"schema":"kiln.review-noise.v1","rules":[...]})`, published in the review summary, change detail and diff annotation. It is deliberately not folded into `binding()` (that would refuse existing journals when an operator edits a display rule) nor `effective_policy()` (that would invalidate queue entries and candidates for a display-only change). Visibility and attribution come from the published identity and the rule ids shown on every hidden group.

### Decision: Server-side classification, display-derived viewed state

**Choice:** the diff response gains `noise: { identity, rules, files: { <displayed path>: <rule id> } }`, computed in the new pure module `code_review/noise.rs` from the exact comparison files (first rule in profile order whose pattern matches the new path or the old path, mirroring how ownership treats renames). The console reads this annotation; it never re-implements glob matching. "Pre-marked viewed" is a display-time derivation from the annotation (`noiseRule(file)`), never a `mark_reviewed`/hunk command and never a stored row, so it applies to every principal and to every revision/base comparison.

### Decision: Domain untouched

**Choice:** `kiln-core` gains no noise concept. `checklist`, `validate_policy`, ownership file facts and queue code never receive the rules. A regression test applies an ownership rule over a noise-matched path and proves submit stays blocked without the owner vote, with the noise profile admitted.

## Risks / Trade-offs

- A broad pattern (for example `**`) hides every file; this is an operator choice, the count and rule ids stay visible and expansion is one click.
- Display identity is not bound into the journal binding, so a rule edit does not show up in historical journals; it is visible on every current response instead.
- The annotation is per displayed comparison; whitespace-filtered or historical comparisons use the same rules.

## Verification Strategy

- **Rust unit tests** (`code_review/noise.rs`): classification for lockfile, vendored and source paths, rename by old path, first-rule precedence; admission rejects invalid patterns (absolute, `..`, character classes, partial `**`), duplicate ids, over-long pattern/id/reason, too many rules; accepts the positive set; identity changes when a rule changes and is absent without rules.
- **Authority regression**: a disposable-repository workspace test with a noise rule over an owned path proves the `owner:*` checklist item stays unmet and submit is rejected without the owner vote, and met with it.
- **Legacy**: a profile without `review` deserializes and re-serializes byte-identically and keeps the same binding and policy identity.
- **Nickel**: positive fixture with noise rules exports; negative fixtures (bad pattern, too many rules, duplicate-free/unknown field) fail.
- **Browser**: disposable instance on 127.0.0.1:9563 with a change touching `Cargo.lock`, `vendor/…` and `src/…`; only the source file is expanded/selected, the hidden count and rule ids are shown, one click expands, light/dark and 320px.
