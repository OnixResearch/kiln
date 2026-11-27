# Fleet Orchestration Specification

## Purpose

Kiln as the claim authority and dispatcher for interactive coding agents across machines (#96).

## Requirements

### Requirement: Claim scopes overlap by containment

r[kiln.fleet.scope] A claim scope MUST be `<owner>/<repo> <path>`, compared case-insensitively on the repository, or `<machine> <path>`. `.` and `system` name the whole target. A repository path MUST be relative, without `..`. Two scopes MUST overlap exactly when they name the same target and either names the whole target, the paths are equal, or one path contains the other at a `/` boundary.

#### Scenario: Containment and case

r[kiln.fleet.scope.scenario.containment]
- GIVEN `OnixResearch/kiln crates/kiln-core`
- WHEN it is compared with `onixresearch/kiln crates`, `OnixResearch/kiln .` and `OnixResearch/kiln crates/kiln-core/src/fleet`
- THEN each MUST overlap
- AND `OnixResearch/kiln crates/kiln-corex`, `OnixResearch/other crates/kiln-core` and `aspen3 system` MUST NOT

#### Scenario: Malformed scopes

r[kiln.fleet.scope.scenario.malformed]
- GIVEN a scope with `..`, an absolute repository path, control characters, a missing path or an oversized field
- WHEN it is parsed
- THEN it MUST be refused with a diagnostic

### Requirement: Kiln grants fenced, leased fleet claims

r[kiln.fleet.claims] Kiln MUST grant a fleet claim only when no overlapping open fleet claim of another holder, active implementation attempt or open foreign swarm claim exists. Each claim MUST carry a unique identity, a monotonic fence and a bounded lease. Renewal and release MUST require the current fence. Expired claims MUST be swept to `Expired`. The same holder re-acquiring the identical scope MUST receive its existing claim.

#### Scenario: Conflicts

r[kiln.fleet.claims.scenario.conflict]
- GIVEN an open claim on `OnixResearch/kiln crates` held by another agent, an executing attempt permitted on `crates/kiln-fleet`, and an open foreign swarm claim on `aspen3 system`
- WHEN agents acquire `OnixResearch/kiln crates/kiln-core`, `OnixResearch/kiln crates/kiln-fleet/src` and `aspen3 kiln.service`
- THEN each MUST receive a conflict naming its source
- AND an acquisition on `OnixResearch/kiln docs` MUST be granted

#### Scenario: Stale fences and expiry

r[kiln.fleet.claims.scenario.lease]
- GIVEN a granted claim
- WHEN it is renewed or released with an older fence, or swept after its lease
- THEN the stale command MUST be refused
- AND the sweep MUST mark it `Expired` and request a release article

### Requirement: Kiln is the swarm writer for fleet work

r[kiln.fleet.mirror] Every granted fleet claim and every released or expired one MUST request exactly one swarm article, in the exact claim form, with a redaction-checked body from named facts and Kiln's own From address. A release MUST reference the claim's message id when known. An unknown post outcome MUST be reconciled by reading the newest window, never by reposting.

#### Scenario: Claim and release articles

r[kiln.fleet.mirror.scenario.articles]
- GIVEN a granted claim followed by its release
- WHEN the mirror effects are performed
- THEN one `CLAIM` and one `RELEASE` article MUST be requested
- AND the `RELEASE` MUST reference the recorded claim message id

### Requirement: Agents reach Kiln through an authenticated fleet API

r[kiln.fleet.api] The fleet API MUST listen only on configured loopback and tailnet addresses. It MUST identify every peer as a configured machine, through loopback or Tailscale identity, and refuse others. It MUST bind each claim's host to that machine. It MUST accept only bounded JSON. Cancellation and decision answers MUST come from operator machines, and a handoff only from the machine hosting the session. The MCP server and client commands MUST call only this API.

#### Scenario: Identity binding

r[kiln.fleet.api.scenario.identity]
- GIVEN a request from an unknown tailnet node, a request declaring another host, and an oversized body
- WHEN they reach the API
- THEN the unknown node MUST be refused, the declared host MUST be replaced by the authenticated machine, and the oversized body MUST be refused

### Requirement: herdr runs interactive attempts under explicit policy

r[kiln.fleet.transport] An implementation attempt MUST run in an interactive agent only through a dispatch plan with `InteractiveWorktree` isolation, the `herdr-interactive` provider profile and work whose operator policy permits `interactive_agent`. The plan MUST declare network and credentials. The herdr adapter MUST create a fresh worktree at the exact base on its chosen machine, start the configured agent kind, run every command as a bounded child process, and report agent status without inventing liveness.

#### Scenario: Interactive plan admission

r[kiln.fleet.transport.scenario.admission]
- GIVEN work with and without `interactive_agent`, and plans with the interactive or Lattice profile
- WHEN plans are admitted
- THEN only the interactive profile on permitted work MUST be admitted with network and credentials
- AND Lattice plans MUST keep their existing rules

#### Scenario: Adapter failures

r[kiln.fleet.transport.scenario.failures]
- GIVEN malformed herdr output, an unsupported kind, an unavailable machine, a deadline and a stale agent name
- WHEN the adapter runs operations
- THEN each MUST be reported as rejected, unavailable or unknown
- AND an unknown mutation MUST be reconciled before any retry

### Requirement: The dispatcher follows sessions to a handoff

r[kiln.fleet.dispatch] The dispatcher MUST take work in Kiln's ranking order, request placement only within free provider slots, renew an attempt's lease only while its session is live, raise one decision for a session blocked past the policy window or idle without a handoff after the allowed nudges, and cancel and stop a session whose start fails or whose agent disappears before capture. Every tick MUST be a pure decision over recorded state and observations.

#### Scenario: Lease and decisions

r[kiln.fleet.dispatch.scenario.lifecycle]
- GIVEN sessions that are working near lease expiry, blocked past the window, idle after work without a handoff, gone, and finished elsewhere
- WHEN a tick is planned
- THEN it MUST request renewal, one decision, a nudge, cancellation with a stop, and a stop respectively
- AND repeating the tick before observations change MUST NOT repeat those effects

### Requirement: Agent commits become ordinary candidate handoffs

r[kiln.fleet.capture] A handoff MUST be captured only for the current attempt and fence, from the session's machine, by fetching the reported commit into the trusted repository, exporting exactly its tree into an isolated root, and admitting it through `materialize_candidate` with a `kiln.herdr.candidate-receipt.v1` receipt. A herdr receipt MUST be refused for non-herdr plans and a Lattice receipt for herdr plans. Verification and publication MUST use only the configured canonical commands.

#### Scenario: Receipt binding

r[kiln.fleet.capture.scenario.receipt]
- GIVEN a herdr plan and a Lattice plan, each with both receipt kinds
- WHEN candidates are materialized
- THEN only the matching receipt MUST be admitted
- AND a receipt for another attempt, fence or workspace MUST be refused

### Requirement: Decisions reach the operator

r[kiln.fleet.decisions] A raised decision MUST record the question, session and attempt, MUST be visible through the API and swarm, MUST accept one answer from an operator machine, and MUST deliver that answer to the session as a prompt. A decision for an ended session MUST close without delivery.

#### Scenario: Answer delivery

r[kiln.fleet.decisions.scenario.answer]
- GIVEN an open decision for a blocked session
- WHEN a non-operator answers, an operator answers, and an operator answers again
- THEN only the first operator answer MUST be recorded and prompted

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

### Requirement: Herdr panes render in a read-only WASM terminal viewer

r[kiln.sdlc.herdr_viewer]
Kiln MUST render an operator-selected Herdr pane's bounded ANSI snapshot in a WebAssembly terminal viewer that interprets escapes only as terminal state. The viewer MUST NOT send input to the pane, and MUST fall back to plain-text capture when the module cannot load.

#### Scenario: Operator views a colored pane

r[kiln.sdlc.herdr_viewer.scenario.view]
- GIVEN a configured host with a pane showing colored output
- WHEN an operator opens the pane in the viewer
- THEN the console renders the pane's cells with their colors
- AND refreshes only while the viewer is open and visible

#### Scenario: Hostile output stays inert

r[kiln.sdlc.herdr_viewer.scenario.inert]
- GIVEN pane output containing HTML, script text, and unsupported escape sequences
- WHEN the viewer renders it
- THEN no markup or script is interpreted
- AND unsupported sequences have no effect outside the cell grid

### Requirement: Fleet composes exact operator observations and controls

r[kiln.fleet.operator_plane] Fleet MUST expose exact attempt inspection, operator attention and recovery, queue and capacity explanation, worker runtime evidence and campaign controls through their existing owners. It MUST preserve explicit unknown and unavailable states, and MUST NOT translate ambiguous dispatch into redispatch or provider observation into lifecycle authority.

#### Scenario: A blocked or unknown attempt can be inspected without crossing authority

r[kiln.fleet.operator_plane.scenario.stuck]
- GIVEN a Cairn change with a blocked or unknown execution
- WHEN an operator inspects it in Fleet
- THEN the exact owner observations and supported controls MUST be reachable
- AND unsupported or unobserved facts MUST be explicit rather than inferred

### Requirement: Herdr window browsing

r[kiln.sdlc.herdr_windows]
Kiln MUST inspect bounded terminal windows on configured Herdr hosts, including windows without a Kiln attempt. Use only configured Herdr hosts and its native window and capture operations. Only an authenticated operator can read terminal contents. Use the selected host and a window observed in that host’s current inventory, bounded sanitized text, and explicit unavailable states.

#### Scenario: Operator uses herdr window browsing

r[kiln.sdlc.herdr_windows.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they open a configured host and select an observed terminal window
- THEN Kiln MUST show the exact host and window with bounded captured output; refuse unknown windows and viewer capture

### Requirement: Queued workload rebind

r[kiln.sdlc.queued_workload_move]
Kiln MUST rebind an exact current unstarted interactive reservation to an explicitly selected admitted host in the native placement owner. It MUST compare the reservation and fence atomically against session binding and transport start, preserve the attempt and work, and retain replayable intent separately from provider observation. Kiln MUST NOT treat quota failover or active migration as a queued rebind.

#### Scenario: Operator moves a queued workload

r[kiln.sdlc.queued_workload_move.scenario.operator]
- GIVEN exact queued work and an enrolled reachable destination with compatible capacity and a repository checkout
- WHEN an operator previews and submits an unchanged move
- THEN only the selected destination may launch; stale, unavailable and potentially started work is refused, and replay or ambiguous observation never creates another execution

### Requirement: Explicit workload placement

r[kiln.sdlc.workload_placement]
Kiln MUST move eligible workloads and split independent units across selected hosts. Keep placement with the owner. Use exact current attempt, checkpoint, destination admission, and fencing for a move, never duplicate an unresolved execution. Split into non-overlapping units with explicit dependencies and destination admission. Distinguish plan, requested effect, and observed placement.

#### Scenario: Operator uses explicit workload placement

r[kiln.sdlc.workload_placement.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they move an eligible attempt or place two independent split units on two enrolled hosts
- THEN Kiln MUST record owner-confirmed placement and lineage; refuse overlap, cycles, stale fences and unavailable destinations

### Requirement: Machine-first Fleet observations

r[kiln.fleet.aggregate_workspace]
The console MUST distinguish enrolled machines from observed Herdr sessions and exact Fleet assignments, retain independent Cairn and issue sources, and NOT infer a healthy or idle host or matched source when the evidence is missing.

#### Scenario: Peer enrollment is not telemetry
- GIVEN a roster with an observed machine, an unreachable target and a peer without a Herdr route
- WHEN the operator opens Fleet
- THEN all three appear with distinct observation states and the work view preserves independently observed Cairn and issue identities

### Requirement: Herdr Core observation isolation

r[kiln.fleet.herdr_core_observation]
Fleet MUST admit Core observations through an explicitly selected versioned local endpoint independently of dispatch, and MUST NOT expose terminal content or infer assignment from Core status or display data.

#### Scenario: A remote line drops
- GIVEN a mapped Core machine with a previously observed terminal
- WHEN the Core connection or its remote line no longer provides a fresh observation
- THEN Fleet MUST distinguish stale or unobserved data from zero live terminals, and retain independent Cairn and issue reads

### Requirement: The Fleet console distinguishes snapshot composition from freshness

r[kiln.fleet.console_snapshot] The authenticated Fleet console MUST label attempt state counts as a breakdown of the listed attempts, distinguish ranked work from the ready-and-unclaimed count, and distinguish the last accepted snapshot from connection and command feedback. A transport failure MUST NOT make retained records appear current. An unrelated update MUST NOT discard a draft answer to a decision that remains open.

#### Scenario: Changes and outages preserve observation meaning

r[kiln.fleet.console_snapshot.scenario.transitions]
- GIVEN a bounded snapshot with mixed attempt states and an open decision
- WHEN a heartbeat, an unrelated update and a connection failure arrive
- THEN the state breakdown MUST count listed attempts only, the draft answer MUST remain, and the last accepted snapshot MUST be marked stale without becoming an empty register

### Requirement: Fleet attempt visualizations drill down locally

r[kiln.fleet.console_drilldown] The Fleet console MUST let operators use keyboard or pointer to filter listed attempts by coordinator state and recorded session machine. The register MUST show the intersection, distinguish no matches from no attempts, and retain the selection across snapshots until cleared. Bar counts MUST still describe the whole listed snapshot.

#### Scenario: Combined filters outlive a changing snapshot

r[kiln.fleet.console_drilldown.scenario.intersection]
- GIVEN listed attempts on two machines and an attempt without a recorded machine
- WHEN the reader selects a state and a machine and the next snapshot has no matching row
- THEN the selected filters MUST remain visible with no matching rows until cleared
- AND clearing MUST restore all rows of the latest snapshot without a request

### Requirement: Fleet shows current Cairn lifecycle progress

r[kiln.fleet.cairn_progress] Fleet MUST refresh the canonical Cairn index while visible independently of its coordinator stream. It MUST show each listed change's observed lane, unit lane counts and exact detail link, distinguish truncated and unobserved results, and mark retained rows stale after a failed refresh rather than inferring progress.

#### Scenario: Cairn progresses without a Fleet event

r[kiln.fleet.cairn_progress.scenario.refresh]
- GIVEN Fleet shows a Cairn change in progress
- WHEN the Cairn index records a later lifecycle lane and then a refresh fails
- THEN the later lane MUST appear without a Fleet event and remain visible as stale after the failure

### Requirement: Cairn progress attributes workers by exact attempt

r[kiln.fleet.cairn_workers] Fleet MUST show the recorded worker binding or matching Fleet session for listed active attempts on current Cairn work. A Fleet session MUST match both attempt and fence. Missing evidence MUST remain unknown, and a dispatch binding MUST NOT imply live placement.

#### Scenario: A neighboring session cannot be substituted

r[kiln.fleet.cairn_workers.scenario.attribution]
- GIVEN an active Cairn attempt with a retained worker binding and Fleet sessions on the current and a different fence
- WHEN Fleet displays the change
- THEN it MUST show the recorded binding and only the current-fence session
- AND an attempt without either observation MUST remain unattributed

### Requirement: Cairn execution context distinguishes plans from observations

r[kiln.fleet.cairn_execution_context] Fleet MUST distinguish the dispatch provider, harness, requested artifact and bound model services from an observed Fleet machine and agent. The display MUST NOT infer a serving model, a Lattice host or swarm membership from the plan or missing observations.

#### Scenario: A plan is not a runtime observation

r[kiln.fleet.cairn_execution_context.scenario.evidence]
- GIVEN an active attempt with planned model evidence and no matching session
- WHEN Fleet displays the attempt
- THEN the model MUST be identified as requested or bound, and machine and swarm membership MUST remain unobserved
- AND a later exact Fleet session MUST populate its machine and agent, not change the model into a runtime claim

### Requirement: Combined Fleet UI surface and proxy exposure of existing metrics and version routes

r[kiln.fleet.unified_console_surface]
Kiln MUST NOT maintain a second divergent interactive HTML control plane in `crates/kiln-fleet/src/ui.html`. `kiln-fleet`'s `/` and `/ui` routes MUST combine into the unified `kiln-web-service` Factory Control Plane (redirecting to the configured console Fleet URL or returning a clear pointer when unconfigured). `crates/kiln-web-service/src/fleet.rs` MUST extend its `upstream_path` allowlist to proxy existing `kiln-fleet` `/v1/metrics` and `/v1/version` endpoints while continuing to reject unallowlisted or traversal paths.

#### Scenario: Standalone Fleet UI route points to unified console

r[kiln.fleet.unified_console_surface.scenario.combined]
- GIVEN a `kiln-fleet` server configured with the primary `kiln-web-service` console URL
- WHEN an operator requests `GET /` or `GET /ui` on `kiln-fleet`
- THEN the response MUST direct the operator to the unified Factory console rather than serving a separate bespoke `prompt()`/`alert()` HTML application

#### Scenario: Metrics and version proxy enforces allowlist

r[kiln.fleet.unified_console_surface.scenario.proxy]
- GIVEN an authenticated console operator requesting `/api/fleet/metrics` or `/api/fleet/version`
- WHEN `fleet.rs` forwards the request to `kiln-fleet`
- THEN `/v1/metrics` and `/v1/version` MUST be proxied cleanly
- AND unallowlisted paths or path-traversal attempts MUST be rejected before upstream dispatch

### Requirement: Live agent pane observation, machine slot census, foreign claims, and dispatch pause

r[kiln.fleet.live_pane_and_machine_census]
`kiln-fleet` MUST expose authenticated HTTP endpoints for live operator control: `GET /v1/machines` (returning `HerdrInventory`, live `HerdrClient::occupancy()`, explicit `Unavailable(HerdrFailure)` errors, and machine drain state), `POST /v1/machines/{name}/drain`, `GET /v1/attempts/{id}/pane?lines=N` (bounded `N <= 1000`, wrapping `HerdrClient::read`), `POST /v1/attempts/{id}/prompt`, `POST /v1/attempts/{id}/interrupt`, `GET /v1/claims?include=foreign,expired` (exposing `SwarmLink::foreign()`, mirror state, and recently expired claims), `GET /v1/dispatch/status` (exposing `TickReport` deferral reasons and `Running | Paused` state), and `POST /v1/dispatch/pause` / `POST /v1/dispatch/resume`. `crates/kiln-web-service/src/fleet.rs` MUST proxy these routes with strict path and parameter validation.

#### Scenario: Operator inspects blocked agent pane and steers without SSH

r[kiln.fleet.live_pane_and_machine_census.scenario.pane_read]
- GIVEN a running attempt in `blocked_question` state on an available `herdr` host
- WHEN the operator opens the Attempt in the Slide-Over Inspector and fetches `GET /api/fleet/attempts/{id}/pane?lines=200`
- THEN `kiln-fleet` MUST return bounded plain-text pane lines from `HerdrClient::read`
- AND the operator MUST be able to send an answer/prompt or interrupt through the guarded control route

#### Scenario: Offline host and foreign claim conflicts remain explicit

r[kiln.fleet.live_pane_and_machine_census.scenario.offline_and_foreign]
- GIVEN one unreachable `herdr` machine returning `HerdrFailure` and one active `ForeignClaim` blocking dispatch
- WHEN the console loads `GET /api/fleet/machines` and `GET /api/fleet/claims?include=foreign,expired`
- THEN the offline machine MUST display `Unavailable` with its error reason rather than `0/0` idle slots
- AND the blocking `ForeignClaim` and its mirror status MUST be surfaced explicitly

### Requirement: Read-only Fleet local-copy census across configured machines

r[kiln.fleet.local_copy_census]
`kiln-fleet` MUST expose an authenticated read-only endpoint `GET /v1/repositories` (and `crates/kiln-web-service/src/fleet.rs` MUST proxy `GET /api/fleet/repositories` -> `/v1/repositories`) that observes local repository checkouts across configured fleet machines and checkout roots without mutating working trees, running remote fetches, or invoking Radicle mutations. Each observed copy MUST report `machine`, `path`, `vcs` (`git | jj | pijul`), exact `head_commit` (or `null`), `branch_or_bookmark`, `dirty` (`bool | null`), discovered remote URLs/RIDs, active `.cairn/changes` directory count, `observed_at_ms`, and `health`. Unreachable or unconfigured hosts MUST appear explicitly with `health: "unavailable"` and their error diagnostic rather than being omitted or reported as clean.

#### Scenario: Offline fleet host remains explicit alongside healthy local copies

r[kiln.fleet.local_copy_census.scenario.offline_host]
- GIVEN a fleet with local checkouts observed on `britton-desktop` and an unreachable peer host `aspen3`
- WHEN a client fetches `GET /api/fleet/repositories`
- THEN `britton-desktop`'s observed local copies MUST be returned with exact HEAD, VCS, remotes, and Cairn count
- AND `aspen3` MUST be reported explicitly as `unavailable` with its failure reason
