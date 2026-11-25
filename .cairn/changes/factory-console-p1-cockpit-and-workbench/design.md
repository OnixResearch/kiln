## Context

P0 establishes a reconciled baseline with guarded dialogs, combined Fleet UI routing, and initial backend projections. However, the console still has 23 top-level destinations, fragmented attention inboxes, vertical 9-panel scroll on Fleet, split polling/SSE transports that risk thread and socket exhaustion (`server.rs` 1-OS-thread-per-connection; `kiln-fleet` `MAX_CONNECTIONS = 32`), blind `blocked_question` handling without live `herdr` panes, a limited `Ctrl+K` palette, and a 10-panel scrolling Code Review aside with a flat queue list.

## Decisions

### Decision: Consolidate 23 rail links to 7 destinations with legacy `?page=` aliases and `?inspect=` deep-links

**Choice:** Update `docs/console-port-inventory.json`, `web.html`, `ui.js`, and `tests/live-console-ui.test.mjs` in lockstep to expose 7 primary rail destinations (`Factory`, `Attention`, `Delivery & Cairn`, `Runs & Evidence`, `Code Review & Queue`, `Fleet & Workers`, `System & Source`) plus footer (`Guide`, `Settings`), mapping all legacy `?page=` values into sub-tab state inside `web.js:readRoute()` and adding `?inspect=<kind>:<id>` for Slide-Over deep links.

**Rationale:** Cuts top-level cognitive load by 70%, unifies `factory-*.js` (`sxvupyosvrrq`) cleanly without parallel panel frameworks, and preserves every existing bookmark and PR link.

### Decision: Exceptions-first 2-region Factory Cockpit with a non-destructive `44rem` Slide-Over Inspector

**Choice:** Render `1.1 Factory` inside `.page-frame-wide` (`110rem`) with a Global Live Strip + Telemetry Ribbon, a main column (`Attention & Escalation Inbox` auto-collapsing to a 1-line healthy bar at 0 items + compact `Delivery Spine` rows + 1-line-per-target `Speculative Merge Train` strip), a `24rem` side rail (`Machine & Slot Census` + `Expiring/Foreign Claims & Fair-Admission Budget`, collapsing healthy non-expiring claims behind a count), and a right-edge `44rem` Slide-Over Inspector that opens without unmounting page DOM or dropping SSE.

**Rationale:** Prevents recreating `fleet.js`'s 9-panel vertical scroll while keeping deep Cairn DAGs (`2.1 Delivery & Cairn`), full Run diagnostics (`2.2 Runs & Evidence`), and 3-pane Diffs (`2.3 Code Review & Queue`) one click away via `Open full view`.

### Decision: Multiplexed `/api/events` SSE stream with a single shared coordinator watcher and visible-only pane tailing

**Choice:** Unify CI `/api/live` and `/api/fleet/events` into one persistent multiplexed SSE endpoint (`/api/events`, emitting typed `ci`, `fleet`, `cairn`, and `attention` frames) fed by a single shared background coordinator watcher in `kiln-web-service`. Serve `GET /api/fleet/attempts/{id}/pane?lines=200` strictly on demand while that Attempt's Slide-Over is open and `document.visibilityState === "visible"`.

**Rationale:** Eliminates 1.5s CI short-polling and 15s Cairn poll lag while preventing multi-tab browser sessions from exhausting `kiln-web-service`'s blocking thread pool or `kiln-fleet`'s 32-connection sync TCP cap.

### Decision: Tiered Control Safety Model with full-factory `Ctrl+K` verbs and 1-click CLI parity

**Choice:** Standardize every control across Factory, Attention, Fleet, Cairn, Runs, and `Ctrl+K` into three tiers: Tier 1 (reversible inline popovers: `Wake`, `Renew +30m/+60m`, `Reconcile`, `Nudge/Prompt`), Tier 2 (`#control-dialog` pinned to exact `source_commit`/fence + freshness guard: `Approve Untrusted Run`, `Answer Decision`, `Pause/Continue`, `Rerun`), and Tier 3 (reason-required modal + explicit confirm + `request_id` idempotency + copyable `kiln-fleet`/`kiln-implementer` CLI command: `Cancel`, `Force-Release/Steal Foreign Claim`, `Drain Machine`, bulk mutations).

**Rationale:** Makes routine operator steering fast and keyboard-accessible while guaranteeing destructive and authority-crossing actions remain auditable, idempotent, and exact-identity bound.

### Decision: 3-Pane Code Review Workbench with adaptive split-mode collapse and list `ci_summary`

**Choice:** Lay out Code Review inside `.page-frame-wide` (`110rem`) with a sticky `16rem` file+noise rail, central diff with inline anchored thread rows, and a `23rem` 4-tab sticky aside (`Checklist & Votes | CI & Previews | Queue & Stack | History & Ops`). Auto-collapse the file rail into a toggleable drawer when in `split` diff mode or below `90rem`. Add a first-class `Merge Queue` tab rendering `Chain` predecessor graphs, active `WindowPolicy` depth, and `superseded_chains` ejection causes, an expandable `jj` Stack Rail above the diff, and lightweight `ci_summary` in `GET /api/code-review` list responses.

**Rationale:** Keeps unified diffs readable at ~68rem and split diffs wide at ~84rem, eliminates scrolling past entire files to find anchored comments, exposes Kiln's speculative merge train visually, and avoids N+1 detail fetches on the change list.

## Risks / Trade-offs

- Live `herdr` pane transcripts may contain arbitrary terminal output or secrets printed by tools; pane reads MUST stay bounded (`MAX_READ_LINES <= 200` default, hard-capped at `1000`), plain-text escaped (no raw HTML/ANSI injection), and authenticated identically to other Fleet routes.
- Updating `docs/console-port-inventory.json` and `tests/live-console-ui.test.mjs` touches cohort tracking for downstream Rats/Meteorite (#68) work; every legacy `?page=` alias and updated inventory entry must be verified by CI in the same change.
- Multiplexed SSE requires clean disconnect cleanup so abandoned tabs release server threads promptly within `profile.bounds.max_connections`.
