## Context

Work records are service-signed native COBs reduced by the pure `work/model.rs` and stored by `work/store.rs`, which already enforces write admission (`write_principals`), request-ID replay (exact duplicates return the original result; changed replays are rejected), expected-head conflicts and global bounds (1,024 objects). Views (`Kind::View`) carry `View { filters, sort, group_by, layout, shared }`; private views are visible only to their owner and visibility is immutable. Teams carry `workflow`, additional statuses on top of seven built-ins. `work.js` fetches the whole bounded snapshot from `/api/work`, filters it in the browser and, for `layout: board`, groups the matching records by status, so empty and unused workflow statuses never appear as columns and there is no move action.

## Goals and Success Contract

A saved view is the board. Its identity, filters, sort and private/shared visibility drive the board and the list alike, and survive reload and list/board switching. Columns are deterministic: configured order when set, otherwise a documented default from built-in and team workflows. Empty columns stay visible. No matching issue is hidden: issues whose status has no column appear in an "Other statuses" lane, and columns whose status no team defines any more are marked retired. A move changes exactly one issue's canonical status through the existing command path. A stale move is shown as a conflict and resolved by the user after a reload; a retry of an uncertain request reuses its request ID and never duplicates the move. The board states how much of the backlog it shows.

False completion includes:
- board columns derived only from the cards that happen to match;
- a move that rewrites other issue fields, writes to the view, or stores card state on the board;
- a retry that issues a new request ID for an outcome that may already have applied;
- silently reloading over a conflict or overwriting a newer head;
- list and board disagreeing about which issues match;
- a filtered or truncated board presented without completeness facts;
- old saved views failing to read, replay or render.

## Decisions

### Decision: Board configuration lives on the existing view payload

**Choice:** `View` gains `board: Option<Board>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`; `Board { columns: Vec<String> }` with `deny_unknown_fields`. `None` (all existing views) means default columns; `Some` must list 1..=`MAX_BOARD_COLUMNS` (64) distinct, bounded status names. Old envelopes deserialize unchanged and new envelopes for unconfigured views are byte-identical to before. Column statuses must be built-in or defined by some team workflow when the board configuration is created or changed; unchanged configuration is not revalidated, so a team may still retire a status that a board names and later edits of the view's other fields are not blocked. Board validation is not part of inbound team revalidation, so board columns never block a team workflow edit. Ownership, sharing and visibility rules are unchanged.

### Decision: Pure board core with documented default order

**Choice:** a new pure module `work/board.rs`. Default columns are the open built-ins (`triage`, `backlog`, `started`), then team-defined statuses, then the closed or parked built-ins (`completed`, `cancelled`, `snoozed`, `duplicate`). Team statuses come from the view's `team` filter when present, otherwise from every team (mixed-team), teams ordered by title then ID, each team's workflow order kept, duplicates merged. Each column reports `builtin`, the IDs of teams defining it and `retired` (neither built-in nor defined by any team). Derived columns are capped at `MAX_BOARD_COLUMNS`; overflow is reported and its issues fall into the Other lane. Within a column cards follow the view sort, ties by ID.

### Decision: Bounded read-only board projection endpoint

**Choice:** `GET /api/work/board` accepts `view` (exact object ID) and the same URL overrides as the Work page (`kind`, `status`, `assignee`, `project`, `q`) plus `sort`. Overrides replace saved filters key by key, exactly as `work.js` does. A missing or another principal's private view is `404 work_object_missing`; unknown or malformed parameters are `400 work_query_invalid`. The projection includes the view ID and head, effective filters, columns with totals and at most `MAX_BOARD_CARDS` (100) cards each, the Other lane, `matched_issues`, `shown_cards`, `list_only` (matching non-issue records the list shows but the board does not), `snapshot_objects`, `object_limit` and `complete`. Cards carry the issue head used as the move's expected head. Only issues become cards. The endpoint does no writes.

### Decision: `set_status` is the only board write

**Choice:** a new `Action::SetStatus { status }`. It applies only to issues, rejects a no-op, rejects `duplicate` (which needs a target), clears `duplicate_of` when leaving `duplicate`, and then runs the existing full validation: the status must be built-in or in the issue's team workflow, snooze needs a future due time. It goes through the unchanged store path: writer admission, visibility, request replay, expected head, bounds and signing. The view is never written by a move.

### Decision: Shared filter semantics and fixture parity

**Choice:** the list's `matches` moves from `work.js` into the pure `work-board.js` (`window.KilnWorkBoard`), which also holds board helpers: query building, projection admission, move targets, failure classification, conflict resolution and completeness wording. The Rust projection implements the same filter semantics. One JSON fixture under `tests/fixtures/work-board/` is checked by both the Rust tests and the node tests, so list and board eligibility cannot drift silently.

### Decision: Accessible move with explicit conflict handling

**Choice:** every card has a 44px "Move…" button and the `m` shortcut, opening a dialog with a radio group of columns (invalid targets disabled with a reason), the exact object, expected head and from → to. Dragging a card onto a column opens the same dialog with that target selected; Escape or dropping outside cancels and sends nothing. Commit sends `set_status` with a request ID kept for that attempt. On `409 work_head_conflict` the dialog shows the conflict and offers "Reload current state"; after reloading it shows the current status and head and lets the user either apply the move to the current head with a new request or keep the current status; if the issue already has the target status it says so and sends nothing. On a timeout, network failure or `503` the outcome is unknown: "Retry same request" resends the identical command, which the store replays. `400`/`403` are final and show the reason. No optimistic state is shown; the board re-reads after each outcome.

## Risks / Trade-offs

- Filter semantics are implemented in Rust and JavaScript. The shared fixture detects drift; locale-specific lower-casing may still differ for rare scripts.
- The board needs a second read besides `/api/work`; cards come from the board projection only, so one board render is internally consistent.
- Mixed-team boards can be wide; columns are bounded and narrow screens stack columns vertically.
- Old views whose `group_by` was not `status` now show status columns on the Work board; `group_by` keeps applying to the list layout.

## Verification Strategy

- **Model unit tests:** board schema bounds, duplicate/unknown columns, unchanged-config tolerance, old view JSON without `board`, `set_status` validation (team workflow, mixed teams, non-issue, no-op, duplicate, snooze).
- **Board unit tests:** default and configured column derivation, mixed-team columns, retired columns, Other lane, card bounds and completeness, sort, the shared eligibility fixture.
- **Integration tests (`tests/work.rs`)** on real disposable native stores: save/reload/share a board view, projection equality with list eligibility, move persisting one event and appearing in every including view with the view head unchanged, unauthorized writer, Worker role and other principal's private view, stale head, exact duplicate retry and changed replay, concurrent competing workspaces, and an old pre-board view history.
- **Node tests:** pure board helpers and the shared fixture.
- **Browser smoke** on a disposable instance at 1440 and 320 px: keyboard move, drag move, cancel, failed request, stale conflict; record counts and overflow measurements in evidence.
