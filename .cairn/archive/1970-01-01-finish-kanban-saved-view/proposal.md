## Why

GitHub issue #4 asks to finish Kanban as a saved view over the existing work issue COBs. Today a saved view can select `layout: board`, but the board only groups the records that currently match, so empty workflow columns disappear, column order follows record order, team-defined statuses are not columns until some issue uses them, and there is no way to move a card. The issue stays the canonical task; the board must be configuration on the existing view COB, not a second task store.

## What Changes

- Extend the existing view COB payload with an optional `board` configuration: an ordered, bounded list of status columns. Old views without it keep reading and replaying unchanged and use deterministic default columns derived from built-in statuses and team workflows.
- Add a pure board core that derives ordered columns, including empty, team-defined, mixed-team and retired columns, and an "Other statuses" lane so no matching issue is hidden.
- Add a read-only bounded `GET /api/work/board` projection for a saved view or the current Work filters, with per-column card bounds and explicit completeness facts.
- Add a `set_status` work command that changes only an issue's canonical status, validated against the issue's team workflow, through the existing authorization, request/replay identity and expected-head path.
- Render the Kanban board on the Work page with an accessible Move dialog, keyboard shortcut and HTML drag-and-drop as an equivalent path, explicit failure, same-request retry and stale-head conflict resolution with reload.
- Move the list filter semantics into a pure browser helper shared by list and board rendering, and prove list/board eligibility parity with one fixture consumed by Rust and node tests.
- Document that board status is planning state, separate from agent/worker execution leases and from CI or release acceptance.

## Impact

- **Consumers:** authenticated work principals using the live Work page; operators reading the work documentation.
- **Owners:** Kiln web-service maintainers own the work model, store shell, board projection and console assets.
- **Durable contribution:** one canonical issue status per card, a bounded board projection with completeness facts, and an explicit stale-write resolution path.
- **Affected:**
  - `crates/kiln-web-service/src/work/` (model, new pure board core), `work.rs`;
  - console assets `work.js`, new `work-board.js`, `work.css`, `web.html`, static routing in `server.rs`;
  - Rust tests in `crates/kiln-web-service/tests/work.rs` and model unit tests, node tests in `tests/live-console-ui.test.mjs`, shared fixtures under `crates/kiln-web-service/tests/fixtures/work-board/`;
  - `docs/operator.md`.

## Out of Scope

- WIP indicators or limits and per-view manual card ordering (the issue's optional follow-ups). Cards stay ordered by the view's sort; no card order is stored on the view.
- Any new storage subsystem, offline multi-writer CRDT or automatic conflict merge. The exact-head concurrency contract is unchanged.
- Moving requests, planning records or views between columns; only issues move. Moving into `duplicate` (needs a target) stays in the issue form.
- Kanban columns on the Inbox, Planning, Triage and Saved views pages; those keep their existing grouped layout.
- Granting execution, CI, merge or release authority through any status.
