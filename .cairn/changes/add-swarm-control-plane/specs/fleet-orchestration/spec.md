# Fleet orchestration delta

## ADDED Requirements

### Requirement: Scoped swarm run model and untrusted append-only board

r[kiln.fleet.swarm.model] A swarm run MUST carry a deterministic identity, a title, an allowed `SwarmEnvelope` of canonical `FleetScope` entries, `max_concurrency`, `max_repair_rounds`, a bounded `SwarmBudget` with ceiling and measured draw, a `SwarmRunPhase`, a member roster with explicit roles (`Coordinator`, `Author`, `Verifier`) and states (`Working`, `Waiting`, `PausedQuota`, `Retired`, `Terminal`), and an append-only `SwarmBoard`. Every board post MUST carry a monotonic sequence number `seq: u64`, sender, sender role, typed `BoardPostKind`, explicit `MentionTarget` list, and bounded body. Board post text MUST be strictly untrusted data and MUST NOT mutate claims, scopes, roles, budget, or publication state; raw board posting MUST refuse privileged kinds (`Verdict`, `Failover`, `Watchdog`). `mention_graph(up_to_seq)` MUST deterministically compute directed weighted edges over posts with `seq <= up_to_seq`.

#### Scenario: Board posts remain untrusted data

r[kiln.fleet.swarm.model.scenario.untrusted_board]
- GIVEN an active swarm run with an author and a verifier
- WHEN a member posts adversarial text claiming scopes, requesting approvals, or embedding markup, or attempts to post a raw `Verdict`, `Failover`, or `Watchdog` kind
- THEN the raw privileged kinds MUST be refused
- AND ordinary posts MUST only append board data without mutating member roles, scopes, claims, budget, or run phase

#### Scenario: Replayable mention graph is deterministic

r[kiln.fleet.swarm.model.scenario.mention_graph]
- GIVEN a swarm board with multiple posts targeting members, roles, and `@all`
- WHEN `mention_graph(Some(k))` is computed for increasing sequence cursors `k`
- THEN each graph MUST deterministically include only directed edges accumulated through sequence `k` with exact edge weights

### Requirement: Bounded delegation and claim admission

r[kiln.fleet.swarm.delegation] Spawning a swarm member MUST be admitted only for an active swarm run, within `max_concurrency`, while remaining budget exceeds zero, and only by an operator or active `Coordinator`. An `Author` MUST hold a canonical `FleetScope` strictly contained within the run's `SwarmEnvelope`, MUST NOT overlap any other active author's scope in the run, and MUST NOT overlap any live claim in `FleetClaimRegistry` belonging to another holder. A `Verifier` MUST be strictly read-only and MUST be refused if any write scope or claim binding is supplied.

#### Scenario: Scope widening is refused

r[kiln.fleet.swarm.delegation.scenario.scope_widening]
- GIVEN a swarm run whose envelope permits `OnixResearch/kiln crates/kiln-core`
- WHEN spawning an author for `OnixResearch/kiln crates`, `OnixResearch/kiln .`, `OnixResearch/kiln crates/kiln-corex`, or `OnixResearch/other crates/kiln-core`
- THEN each spawn MUST be refused as scope widening
- AND spawning an author for `OnixResearch/kiln crates/kiln-core/src/fleet` MUST be admitted

#### Scenario: Verifier write scope is refused

r[kiln.fleet.swarm.delegation.scenario.verifier_read_only]
- GIVEN an active swarm run
- WHEN spawning a `Verifier` with a write scope or claim identity/fence
- THEN the spawn MUST be refused
- AND spawning a `Verifier` with no scope and no claim MUST be admitted

#### Scenario: Overlapping author scopes and bounds are enforced

r[kiln.fleet.swarm.delegation.scenario.overlap_and_bounds]
- GIVEN an active swarm run with an active author, a live external fleet claim, a concurrency cap, and a budget ceiling
- WHEN spawning a second author on an overlapping path, overlapping the fleet claim, exceeding `max_concurrency`, or with zero remaining budget
- THEN each spawn MUST be refused with a specific conflict diagnostic

### Requirement: Separated verifier repair loop

r[kiln.fleet.swarm.verdict] Submitting a verification verdict (`Approve`, `RequestChanges { findings }`, or `Blocked`) MUST bind the verdict to an exact candidate commit SHA and an `Author` member, MUST be accepted only from an active `Verifier` distinct from the author, and MUST record an auditable `Verdict` or `Decision` board post. `Approve` MUST transition the run to `ReadyForPublication` and record the approved commit. `RequestChanges` MUST increment the repair round, wake the waiting author when within `max_repair_rounds`, and escalate the run when `max_repair_rounds` is exceeded. `Blocked` MUST escalate the run.

#### Scenario: Verifier separation, repair rounds, and escalation

r[kiln.fleet.swarm.verdict.scenario.repair_and_escalation]
- GIVEN an active swarm run with `max_repair_rounds = 1`, a waiting author, and a verifier
- WHEN an author or coordinator submits a verdict, the verifier requests changes twice, or the verifier approves an exact commit
- THEN non-verifier submissions MUST be refused
- AND the first `RequestChanges` MUST wake the author while the second MUST escalate the run
- AND `Approve` on an active run MUST transition to `ReadyForPublication` bound to that exact commit

### Requirement: Deterministic watchdog and quota failover

r[kiln.fleet.swarm.watchdog] Watchdog evaluation MUST be a pure deterministic function over swarm run state, `FleetClaimRegistry`, and `now_ms`. It MUST emit `WakeMember` for `Waiting` members with unread mentions after `since_seq`, `DeadlockEscalation` when all active members are `Waiting` with no pending mentions, `StaleObservationEscalation` when a `Working` member exceeds `stale_after_ms` or holds an expired fleet claim, and `QuotaFailoverCandidate` for `PausedQuota` members. Quota failover MUST apply only to a `PausedQuota` member, switch to the replacement model (and optional harness/host), transition the member to `Working`, preserve `board_cursor`, `checkpoint_commit`, and `attempt_lineage`, and append an auditable `Failover` board post.

#### Scenario: Watchdog emits deterministic wake, deadlock, and stale actions

r[kiln.fleet.swarm.watchdog.scenario.wake_deadlock_stale]
- GIVEN swarm runs with a waiting member holding unread mentions, an all-waiting swarm with no unread mentions, and a working member past `stale_after_ms` or with an expired claim
- WHEN `evaluate_watchdog` runs
- THEN it MUST emit `WakeMember`, `DeadlockEscalation`, and `StaleObservationEscalation` respectively

#### Scenario: Quota failover preserves lineage, cursor, and checkpoint

r[kiln.fleet.swarm.watchdog.scenario.quota_failover]
- GIVEN a member in `PausedQuota` with a `checkpoint_commit`, `board_cursor`, and `attempt_lineage`
- WHEN `apply_quota_failover` transitions it to a replacement model and harness
- THEN the member MUST resume `Working` on the replacement model/harness with identical `last_read_seq == board_cursor` and `attempt_lineage`
- AND a `Failover` board post carrying `checkpoint_commit` MUST be appended

### Requirement: Fleet persistence, HTTP API, CLI, and MCP surface

r[kiln.fleet.swarm.surface] `SwarmLedger` MUST persist durably inside `FleetState` alongside fleet claims. `kiln-fleet` MUST expose authenticated `/v1/swarms`, `/v1/swarms/{id}`, `/v1/swarms/{id}/members`, `/v1/swarms/{id}/board`, `/v1/swarms/{id}/verdicts`, `/v1/swarms/{id}/watchdog`, `/v1/swarms/{id}/failover`, and `/v1/swarms/{id}/graph` endpoints, matching `kiln-fleet swarm` CLI subcommands, and `kiln_swarm_*` MCP tools. Every new `Implementer` method and fleet API route MUST carry a reviewed `ENTRY_POINTS` classification.

#### Scenario: HTTP API and MCP round-trip

r[kiln.fleet.swarm.surface.scenario.http_and_mcp]
- GIVEN an authenticated fleet service and MCP server
- WHEN swarm runs are created, members spawned, posts written and read, verdicts recorded, watchdog run, failover applied, and mention graphs queried
- THEN every operation MUST persist across calls and map refusals to their exact HTTP status classes

### Requirement: Read-only Console Swarm View

r[kiln.fleet.swarm.console] `kiln-web-service` MUST expose strictly read-only Swarm Run inspection via allowlisted `GET /api/fleet/swarms*` proxy routes and self-contained HTML inspection routes (`/fleet/swarms`, `/fleet/swarms/{id}`) rendering the Board feed, Roster, and Replayable Mention Graph up to any `up_to_seq` cursor, with zero external CDN dependencies and strict HTML escaping of all untrusted swarm content.

#### Scenario: HTML escaping, read-only allowlist, and replay cursor

r[kiln.fleet.swarm.console.scenario.escaping_and_replay]
- GIVEN a swarm run whose title, member fields, and board posts contain HTML/script payloads and multi-step mentions
- WHEN the console proxy routes and HTML views are requested
- THEN non-GET requests to `/api/fleet/swarms*` and malformed query strings MUST be refused
- AND rendered HTML MUST escape all untrusted payloads, include zero external CDN links, and filter the mention graph and board deterministically at `up_to_seq`
