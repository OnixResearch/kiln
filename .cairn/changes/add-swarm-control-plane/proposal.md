## Why

Issue [#118](https://github.com/OnixResearch/kiln/issues/118) introduces Kiln Swarm Orchestration: scoped run boards, bounded delegation, separated `@verifier` repair loops, a deterministic watchdog, quota failover, and a read-only Console Swarm View. Multi-agent collaboration patterns like shared append-only boards, `@mention` wakeups, dedicated verification loops, stall detection, and quota-driven model switching are high-leverage, but prompt-only boundaries inside a single harness cannot enforce scope containment, claim exclusivity, read-only verifier separation, or budget caps across heterogeneous agents (OMP, Claude Code, Codex, Antigravity) and hosts.

Kiln already owns deterministic fleet claims (#96), separated verification authority (#46, #69), stale-observation fencing (#105), and measured budget tracking (#98, #115). Layering a pure, fail-closed swarm control plane on top of Kiln's fleet architecture gives every harness a single authoritative run board, bounded delegation, verifiable repair loops, and replayable console observability without ever letting agents widen their own authority or publish directly.

## What Changes

- **Pure `kiln_core::fleet::swarm` control plane**: adds `SwarmRun`, `SwarmRunConfig`, `SwarmEnvelope`, `SwarmBudget`, `SwarmRole` (`Coordinator`, `Author`, `Verifier`), `SwarmMember`, `SwarmMemberState` (`Working`, `Waiting`, `PausedQuota`, `Retired`, `Terminal`), and `SwarmBoard` with monotonic `seq`, typed `BoardPostKind`, explicit `MentionTarget`, per-member `last_read_seq` cursors, `read_since`/`read_seqs`, and deterministic `mention_graph(up_to_seq)` edges.
- **Bounded delegation & claim admission**: `admit_spawn` deterministically enforces parent `SwarmEnvelope` containment via directional `contains_scope` (refusing any scope widening), `max_concurrency`, remaining budget > 0, `Verifier` read-only exclusion (no write scope or claim), and non-overlapping `Author` write scopes against both active swarm authors and `FleetClaimRegistry`.
- **Separated `@verifier` repair loop**: `submit_verdict` binds `Approve`, `RequestChanges { findings }`, and `Blocked` verdicts to an exact candidate commit SHA and author member, enforces `Verifier`-only submission, tracks `repair_round`, transitions to `ReadyForPublication` on `Approve`, wakes the author within `max_repair_rounds`, and escalates on round exhaustion or `Blocked`.
- **Deterministic watchdog & quota failover**: `evaluate_watchdog` and `record_watchdog_pass` detect unread `@mention` wakeups, all-waiting deadlocks, stale observations or expired claims, and `PausedQuota` failover candidates without any LLM or I/O. `apply_quota_failover` transitions a paused member to a replacement model/harness/host while preserving `board_cursor`, `checkpoint_commit`, and `attempt_lineage`, posting an auditable `Failover` board entry.
- **Durable persistence, HTTP API, CLI & MCP surface**: extends `FleetState` in `kiln-implementer` with backward-compatible `#[serde(default)] swarms: SwarmLedger`, registers every new `Implementer` method and `/v1/swarms*` route in `ENTRY_POINTS`, exposes `/v1/swarms*` HTTP routes, `kiln-fleet swarm ...` CLI subcommands, and `kiln_swarm_*` MCP tools.
- **Console Swarm View (`kiln-web-service`)**: adds strictly allowlisted read-only `/api/fleet/swarms*` proxy routes, server-rendered HTML inspection views (`/fleet/swarms`, `/fleet/swarms/{id}?up_to_seq=...`) with strict HTML escaping and zero external CDN dependencies, and live swarm inspection in `fleet.js`.

## Capabilities

### Modified Capabilities

- `fleet-orchestration`: adds scoped swarm runs, append-only run boards with mention graphs, bounded worker delegation, separated verifier repair loops, deterministic watchdog/failover, fleet API/CLI/MCP surfaces, and read-only console swarm views.

## Impact

Adds `crates/kiln-core/src/fleet/swarm/`, extends `crates/kiln-core/src/fleet/scope.rs`, `crates/kiln-core/src/fleet/mod.rs`, `crates/kiln-core/src/implementation/authority/entry_points.rs`, `crates/kiln-implementer/src/fleet.rs`, `crates/kiln-fleet/src/{http,store,api,cli,mcp}.rs`, and `crates/kiln-web-service/src/{fleet.rs,swarm_view.rs,server.rs,fleet.js,fleet.css}`. Existing fleet claims, sessions, decisions, and persisted `kiln.fleet-state.v1` rows remain backward-compatible.

## Non-goals

Automatic remote git publication or merging from swarm verdicts (remains governed by Patchgate and collaboration-provider shells), LLM-based watchdog agents, and interactive mutation buttons in the read-only Console Swarm View are outside this change.
