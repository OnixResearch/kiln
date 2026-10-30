## Context

The change-review domain lives in `kiln-core::change_review` (`no_std + alloc`). `Revision` is one immutable patch set with `author` and `created_ms`. `Vote` binds an actor and value to one revision id. `Thread` carries an immutable `Anchor` (revision, file, side, lines) and comments. `SubmitOutcome` records `submitted_ms`, and `OwnershipPolicy` rules map path patterns to owners.

The web shell (`kiln-web-service/src/code_review*`) persists a private SQLite state document and a per-request journal. Before any provider effect, it stores a `Pending` entry whose `Completion::Register` carries the exact `domain::Command::{Create, Upload}`, and reconciliation replays it. Branch contribution intake (`Action::Contribute`) rewrites to `Create` or `Upload`. The request digest is BLAKE3 over the serialized `Request`, so serialization changes must not alter the bytes of existing requests.

`kiln-observe-core` projects a CI `State` plus an explicit `ObservationWindow` (events or log) into bounded `ObservationView`s with `kiln.observe.view.v1` identities. Its shells add transport only:
- the HTTPS shell maps routes onto a `ViewRequest`;
- the iroh shell carries the `ObservationRequest` JSON;
- the offline dashboard renders projected views.

Review state never enters the CI journal, so review views need their own admitted window.

## Goals and Success Contract

- A declared authorship is accepted on create, contribute and upload, and is journaled on the patch set.
- Absent authorship replays as `undeclared`, and existing request digests and state bytes stay unchanged.
- Invalid authorship fails before mutation or provider effect.
- For a fixture review journal with known human and agent changes, `review_friction` reports the exact approval latency, patch-set and suggestion distributions, and the merged-without-suggestion share. `suggestion_anchors` reports the exact ranked anchors.
- The HTTPS, iroh and dashboard legs produce byte-identical views and identities for the same request and window.
- Window, paging and bound violations fail closed with named diagnostics.

False completion includes:
- inferring authorship from any fact;
- changing existing view or profile identities;
- a shell computing its own statistics;
- a view or export that can mutate review state.

## Decisions

### Decision: Authorship is a per-patch-set declared value in the review core

**Choice:** `kiln_core::change_review::Authorship` is an internally tagged enum: `{"kind":"undeclared"|"human"|"mixed"}` or `{"kind":"agent","harness":..,"model":..?}`. `Revision.authorship` uses `#[serde(default, skip_serializing_if = "Authorship::is_undeclared")]`. The same encoding is optional on the web `Create`, `Contribute` and `Upload` actions.

`validate_authorship` rejects:
- empty or untrimmed text;
- control characters;
- harness or model text over `MAX_AUTHORSHIP_TEXT_BYTES` (128).

`apply` runs it for `Create` and `Upload`. The web shell rejects the same cases first, with `400 review_authorship_invalid`.

Service-created patch sets carry `undeclared`. These are rebase, revert, cherry-pick and applied suggestions.

`authorship_class(change)` folds declared patch sets only:
- no declared patch set gives `undeclared`;
- one shared class gives that class;
- differing declared classes give `mixed`.

**Rationale:** The patch set is the immutable unit that already travels in the journaled `Command`. Skipping the default keeps old request digests and state bytes identical, and serde defaults make old journals replay as `undeclared`. Folding only declared values never turns silence into a claim.

### Decision: Review facts reach observation as an admitted review window

**Choice:** `ObservationWindow` gains `review: Option<ReviewWindow>`, which is serialized only when present. A `ReviewWindow` has:
- `start_ms` and `end_ms`, a half-open window;
- sorted `changes`, each a `ReviewChangeFact` with `change_id`, `authorship` class, `patch_sets_ms`, an optional `approval { patch_set, approved_ms }`, `suggestions [{ path, owner?, created_ms }]` and an optional `merged_ms`.

Window admission rejects each of these:
- an empty or inverted range;
- a span over `max_review_window_ms`;
- more than `max_review_changes` changes;
- unsorted or duplicate change ids;
- empty, unordered or over-bound patch sets;
- a first patch set outside the window;
- any fact at or after `end_ms`, or before the first patch set;
- an approval that does not bind the latest admitted patch set;
- more than `max_review_suggestions_per_change` suggestions;
- text over `max_text_bytes`.

Review views require a review window and reject event or log windows. All other views reject a review window.

`review_window(state, ownership, start_ms, end_ms)` is a pure derivation over `ReviewState`. It includes changes whose first patch set falls in the window and clips every fact to `< end_ms`.
- **Approval:** the earliest `approve` vote bound to the latest patch set created before `end_ms`.
- **Suggestions:** anchored threads opened before `end_ms` by someone other than the change owner, keyed by anchor file and the first ownership rule that applies to that path and target.
- **Merged:** `submitted_ms` before `end_ms`.

**Rationale:** This follows the existing explicit-window pattern, keeps the core pure, and keeps private review state out of the shells and views.

### Decision: Two view kinds with fixed vocabulary, tags and redaction

**Choice:** New `ViewKind`s `review_friction` (tag 7) and `suggestion_anchors` (tag 8). New `ViewRequest`s `ReviewFriction` and `SuggestionAnchors { cursor, limit }`. New bodies `ReviewFriction` (tag 8) and `SuggestionAnchors` (tag 9).

`review_friction` returns one row per class in fixed order: human, agent, mixed, undeclared. Each row reports:
- `changes` and `approved_changes`;
- distributions (`count`, `total`, `min`, lower `median`, `max`) of approval latency in milliseconds, patch sets before approval, and suggestions per change;
- `merged_changes`, `merged_without_suggestion`, and the integer per-mille share.

`suggestion_anchors` ranks `(path, owner)` by suggestions descending, then distinct changes descending, then path and owner ascending. It is paged by an offset cursor:
- a zero limit fails;
- a limit above `max_page_items` is capped;
- a cursor past the total fails with `cursor_ahead`.

New `ViewField`s `suggestion_paths` and `suggestion_owners` redact anchor text after aggregation and are named in `redacted_fields`.

Profiles declare an optional `bounds.review` (`max_review_changes`, `max_review_window_ms`, `max_review_patch_sets`, `max_review_suggestions_per_change`). Profile admission requires it when a review view is declared. Identity framing appends review bounds and the review window only when present.

**Rationale:** Integers and fixed ordering keep `no_std` identities deterministic. Optional framing keeps every existing profile, view identity and deployed profile document valid.

### Decision: Shells transport, never compute

**Choice:**
- **HTTPS:** routes `GET /v1/views/review-friction` (no query) and `GET /v1/views/suggestion-anchors?cursor=&limit=`.
- **iroh:** carries the same `ObservationRequest`.
- **Snapshot:** `ObservationSnapshot::with_review` supplies the window. A missing window is `404 unknown_target`, and window-admission diagnostics map to `500 invalid_review_window` on HTTPS, like other server-side window faults.
- **Dashboard:** accepts `--review <path>`, a bounded review-window JSON document, and renders both views (first anchor page) in text and HTML with their identities.
- **Web service:** exposes `GET /api/code-review/review-window?start_ms=&end_ms=` to authenticated readers, which returns the derived window. The derivation must pass profile-independent structural checks, and the response is bounded by the existing response limit.

**Rationale:** Every surface calls `project_observation`, so equal inputs produce equal views and identities. The export gives operators a real source for the window without granting authority.

### Decision: Console declares, never infers

**Choice:** The create and contribution dialogs add a labelled `Authorship` select, defaulting to Undeclared, with Human, Agent and Mixed options. Agent reveals labelled Harness (required) and Model (optional) inputs with byte limits. Change detail shows the change class and each patch set's declared authorship, and says it is self-declared. The console uses GPUI Kit tokens and existing field helpers only.

**Rationale:** Explicit declaration at the point of contribution, with a safe default.

## Risks / Trade-offs

- Authorship is self-declared and unverified, so a contributor can misdeclare. The docs and views say so, and the views claim nothing more.
- Approval and suggestion definitions are measurement choices (latest-patch-set approval, non-owner threads, first owning rule). They are documented and fixed by tests; changing them later changes view bodies.
- The dashboard renders only the first anchor page; deeper pages are available over HTTPS and iroh.
- Undeclared rebase or applied-suggestion patch sets do not change a change's class, by design.

## Verification Strategy

- **`kiln-core` unit tests:** declared authorship accepted and stored; old JSON without the field replays `undeclared` and re-serializes byte-identically; invalid kinds, empty or control text and oversized harness or model rejected; class fold.
- **`kiln-observe-core` tests:**
  - a fixture review journal (`fixtures/observation/review-journal-v1.json`) replayed through `apply` derives the checked-in window `fixtures/observation/review-window-v1.json`;
  - the projected friction and anchor views match exact expected latency, patch-set, suggestion and share values;
  - deterministic identity;
  - redaction;
  - negative window, paging, bound and profile cases.
- **`kiln-web-service` tests:** intake journals the declared field into the pending command and change state; legacy pending and state documents replay `undeclared`; invalid authorship is rejected with `400`; window export.
- **HTTPS and iroh tests:** served review views are byte-identical to the direct projection and to each other; negative route, query and missing-window cases.
- **Dashboard tests:** rendered text and HTML carry identical view identities and values.
- **Console:** `node --test tests/live-console-ui.test.mjs`.
- **Profiles:** Nickel exports regenerated and compared with fixtures.
