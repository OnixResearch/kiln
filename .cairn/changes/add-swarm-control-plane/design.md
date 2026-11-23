## Context

`kiln_core::fleet` already implements pure, `#![no_std]` rules for claim scope parsing and overlap (`scope.rs`), fenced leased fleet claims (`claims.rs`), interactive sessions (`session.rs`), and operator decisions (`decision.rs`). `kiln-implementer` persists `FleetState` as one versioned JSON blob in SQLite under immediate transactions, `kiln-fleet` exposes the authenticated `/v1/*` HTTP API, CLI, and MCP stdio server, and `kiln-web-service` projects read-only and operator-gated views over the loopback fleet coordinator.

## Goals and Success Contract

1. Every swarm state transition, admission check, verdict decision, watchdog evaluation, quota failover, and mention-graph projection lives in pure `#![no_std]` `kiln_core::fleet::swarm` functions with explicit `now_ms` inputs and named limits.
2. Board post text is strictly untrusted data: raw posts cannot forge `Verdict`, `Failover`, or `Watchdog` entries and never mutate claims, scopes, roles, budgets, or run phases.
3. Delegation fails closed: scope widening outside `SwarmEnvelope`, any write scope or claim on a `Verifier`, overlapping author scopes (intra-swarm or against live `FleetClaimRegistry` claims), concurrency overflow, and exhausted budgets are deterministically refused.
4. Separated `@verifier` repair loops bind verdicts to an exact commit SHA and author, enforcing `Verifier`-only submission and bounded `max_repair_rounds`.
5. Deterministic watchdog evaluation detects event-driven wakeups, all-waiting deadlocks, stale observations/expired claims, and quota-failover candidates; failover preserves `board_cursor`, `checkpoint_commit`, and `attempt_lineage`.
6. Every `Implementer` method and `/v1/swarms*` fleet API route has a reviewed `ENTRY_POINTS` classification passing `authority_coverage`.
7. The Console Swarm View is strictly read-only, has zero external CDN dependencies, escapes every untrusted string before HTML rendering, and supports deterministic mention-graph replay at any `up_to_seq` cursor.

## Decisions

### Decision: Directional scope containment for `SwarmEnvelope`

**Choice:** add `contains_scope(outer: &FleetScope, inner: &FleetScope) -> bool` to `kiln_core::fleet::scope`, reusing the `/`-boundary prefix rule while requiring `outer` to cover `inner`.

**Rationale:** `overlaps(a, b)` is symmetric, which is right for conflict detection but wrong for delegation admission. A child author requesting `OnixResearch/kiln .` or `OnixResearch/kiln crates` when the parent envelope only allows `OnixResearch/kiln crates/kiln-core` overlaps the envelope but widens authority; `contains_scope` refuses all widening while permitting exact matches and nested subpaths.

### Decision: Backward-compatible `SwarmLedger` inside `FleetState`

**Choice:** add `#[serde(default)] pub swarms: SwarmLedger` to `kiln_implementer::fleet::FleetState`.

**Rationale:** storing `SwarmLedger` inside `FleetState` lets `admit_spawn` and `evaluate_watchdog` read the exact `FleetClaimRegistry` in the same immediate SQLite transaction without cross-table races, preserves backward compatibility with existing `kiln.fleet-state.v1` rows, and automatically bumps `fleet_observation_version` for console live updates.

### Decision: Privileged board post kinds gated by typed transitions

**Choice:** `post_to_board` refuses `BoardPostKind::Verdict`, `BoardPostKind::Failover`, and `BoardPostKind::Watchdog`; those entries are appended exclusively by `submit_verdict`, `apply_quota_failover`, and `record_watchdog_pass`.

**Rationale:** guarantees that untrusted agents posting to `/v1/swarms/{id}/board` can never spoof verifier approvals, failover receipts, or watchdog escalations, reinforcing the untrusted-data invariant structurally rather than by convention.

### Decision: Read-only allowlisted proxy plus self-contained HTML inspection in `kiln-web-service`

**Choice:** extend `kiln_web_service::fleet::upstream_path` with strictly allowlisted `GET` routes for `/api/fleet/swarms*` (validating numeric-only query parameters `up_to_seq`, `since`, `limit`), add `crates/kiln-web-service/src/swarm_view.rs` serving `/fleet/swarms` and `/fleet/swarms/{id}` with an inline SVG mention graph and `<input type="range">` replay form, and render live swarm runs in `fleet.js` using DOM `textContent` only.

**Rationale:** operators and viewers get immediate read-only inspection (Board, Roster, Replayable Mention Graph) both via direct HTML routes (zero CDN, strict escaping, works without JS) and inside the console Fleet page, while mutation authority stays exclusively on the authenticated `kiln-fleet` API.

## Risks / Trade-offs

- Storing `SwarmLedger` inside the `FleetState` JSON blob keeps transactions atomic and simple, bounded by `MAX_SWARM_RUNS` (64) and `MAX_BOARD_POSTS` (4096). Very long-lived archives would eventually compact or prune retired runs in a follow-up.
- Adding query string parsing to `kiln_fleet::http::Request` touches existing `Request` literal sites in tests; keeping `query: Option<String>` explicit ensures zero silent parameter loss.

## Verification Strategy

- Pure unit and adversarial tests in `kiln-core::fleet::swarm`: scope widening refusal, verifier write-scope/claim refusal, overlapping author scopes (intra-swarm and fleet claims), concurrency/budget exhaustion, non-verifier verdict refusal, repair-round exhaustion escalation, `Approve` -> `ReadyForPublication`, unread-mention wakeup, all-waiting deadlock, stale observation/expired claim detection, quota failover preserving `board_cursor`/`checkpoint_commit`/`attempt_lineage`, deterministic `mention_graph(up_to_seq)` replay, and untrusted board-post isolation.
- Store and `authority_coverage` tests in `kiln-implementer`.
- HTTP API, CLI, and MCP end-to-end tests in `kiln-fleet`.
- Proxy allowlist, read-only enforcement, replay cursor, and HTML/XSS escaping tests in `kiln-web-service`.
- `cargo fmt --check`, `cargo clippy -p kiln-core -p kiln-implementer -p kiln-fleet -p kiln-web-service --all-targets -- -D warnings`, `cargo test`, and pinned `cairn validate` + `cairn gate proposal|design|tasks add-swarm-control-plane`.
