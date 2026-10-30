## Context

`kiln_core::change_review` holds the pure review domain: `Change.votes: Vec<Vote>`, `voter()` admission, `checklist_at()` counting eligible current-revision approvals, vetoes and `owner:*` items, `search()` and `SearchQuery` (stored in saved views). The web shell (`crates/kiln-web-service/src/code_review.rs`, `code_review/commands.rs`, `code_review/model.rs`) maps HTTP actions to domain commands; the command route currently requires write principals. The review-window export (`kiln_observe_core::review_window`) derives `ReviewWindow`/`ReviewChangeFact` from `ReviewState`; `project_review_friction` derives `ReviewFrictionView`; view identities use the canonical encoder in `kiln-observe-core/src/identity.rs`. The HTTPS (`kiln-adapter-http-observe`) and iroh (`kiln-adapter-iroh-observe`) shells and `kiln-dashboard` serve or render those views.

## Goals and Success Contract

A read principal who is neither the owner nor an author can cast and withdraw one advisory approval bound to the exact current revision. Across every state reachable with and without advisory approvals, every checklist item and the `ready` flag are identical; eligible reviewer votes behave exactly as before. Advisory counts appear in the review-window export and identically in the HTTPS shell, the iroh shell and the dashboard. Old journals, saved views, review windows and view identities are byte-identical.

False completion includes: an advisory approval stored as a `Vote`; any checklist item flipping; a candidate cleared or created by advisory activity; read principals gaining any other command; a review-window or identity byte change for data without advisory approvals.

## Decisions

### Decision: Separate command and collection

**Choice:** `Command::AdvisoryApprove { change, expected_revision }` and `Command::AdvisoryWithdraw { change, expected_revision }` appended to `Command`, recorded in `Change.advisory: Vec<AdvisoryApproval { actor, revision, created_ms }>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. `VoteValue` is unchanged, so neither `voter()` nor the `eligible` filter can ever admit an advisory record. Re-casting on the same revision replaces the actor's record; withdrawing a missing record fails `advisory_missing`. A named `MAX_ADVISORY_PER_REVISION` bounds records per revision.

### Decision: Who may advise

**Choice:** the domain rejects `advisory_forbidden` when the actor is the change owner or the author of any revision, and `closed` when the change is not open; policy membership is not required. The shell admits exactly `advisory_approve` and `advisory_withdraw` for read-only principals (all read principals see all changes of the profile); all other actions keep `review_write_denied`. The action is journaled like other commands (version bump, request digest).

### Decision: No authority side effects

**Choice:** advisory commands do not touch `votes`, `attention`, `candidate`, `reviewers` or the inbox, and emit no notification, so delivery recipients are unchanged. Historical records stay attributed to their revision and render as historical.

### Decision: Search filter

**Choice:** `SearchQuery.advisory: Option<bool>` (serde default, omitted when `None`): `Some(true)` keeps changes with at least one current-revision advisory approval, `Some(false)` those with none. Query parameter `advisory=with|without`.

### Decision: Observation export and identity

**Choice:** `ReviewChangeFact.advisory_approvals: u32` counts advisory approvals on the latest admitted patch set created before `end_ms`; `AuthorshipFrictionView` gains `advisory_approvals` (total) and `advised_changes`. All are omitted from JSON when zero. Admission bounds `advisory_approvals` by the named `MAX_REVIEW_ADVISORY_PER_CHANGE`. The canonical identity encoder carries them by setting bit 2 on the trailing option tag (`merged_ms` for changes, `merged_without_suggestion_per_mille` for friction rows) and appending the counts, so zero-advisory windows and views keep byte-identical encodings and identities while non-zero values remain injective.

### Decision: Dashboard rendering

**Choice:** the dashboard text and HTML friction sections render advisory totals in a clearly labelled non-binding column/line for every class, including zero, so the same view renders the same everywhere.

## Risks / Trade-offs

- Advisory actions bump the change version like votes, so a concurrently open form may see a version conflict; this keeps one ordering for all change mutations.
- Rejecting authors of any revision is stricter than votes (current author only); it matches "their own change".
- The identity tag-bit extension is a local encoding convention; it is documented next to the encoder and tested for both stability and injectivity.

## Verification Strategy

- **Property-style domain test:** enumerate checklist-relevant state combinations (approvals, vetoes, ownership rules, threads, CI, WIP, dependencies) and assert that adding advisory approvals from every eligible-to-advise principal leaves every checklist item and `ready` unchanged; advisory never admitted as a vote.
- **Domain positive/negative:** cast/withdraw by read principal; owner/author/closed/stale revision/missing withdraw rejected; new revision makes advisory historical; eligible votes and checklist results identical with and without advisory; candidate untouched; no notifications.
- **Legacy:** pre-advisory change, saved query and review-window JSON round-trip byte-identically; view identity of the v1 review fixture unchanged.
- **Shell:** read-only principal may advise but not vote; summary/detail expose advisory lists; `advisory` search filter.
- **Export/parity:** `review_window` exports counts; HTTPS and iroh shells return views equal to the direct projection for an advisory fixture; dashboard text and HTML render the counts.
- **Browser:** disposable instance on 127.0.0.1:9563; list, cast via confirmation, filter, historical after upload; light/dark and 320px.
