## Context

- **Work and attempts.** Kiln admits implementation work from GitHub issues and Cairn snapshots. `AttemptLedger` holds one current attempt per work key, with a bounded lease and a monotonic fence. Execution goes through a reserved `ImplementationDispatchPlan`. Today only Lattice runs plans, as the `lattice-implementation` profile in an `IsolatedSnapshot` workspace with no ambient credentials.
- **Results.** `materialize_candidate` turns an exported workspace plus a Lattice host receipt into a `CandidateHandoff`. Verification (`kiln-web-service verify-implementation`) and publication (`prepare-publication`, `publish`) consume that handoff.
- **Agents.** The interactive agents run in herdr, a terminal multiplexer that recognizes coding agents. Its CLI talks to one headless server per session (`herdr --session NAME server`), or to a saved SSH machine with `--machine LABEL`. It returns JSON for workspaces, worktrees, panes and agents, and it reports agent status as `idle`, `working`, `blocked`, `done` or `unknown`. The full schema (protocol 22) is `herdr api schema --output PATH`.
- **Swarm.** `bots.claims` uses `CLAIM|RELEASE <target> <scope>`. The swarm skill defines `<owner>/<repo> <path>` (`.` for the whole repository) and `<machine> <unit or path>` (`system` for the whole machine). All agents on one machine share that machine's account and From address.
- **Machines.** Tailscale connects them; `tailscale whois --json IP` names the peer node without root.

## Decisions

### 1. Kiln owns claims and ordering; providers own placement

The fleet core decides which work runs next (Kiln's existing ranking through `claim_next_deferred`), which claims conflict, when leases renew and when to ask a human. The herdr adapter chooses the machine and slot from its own inventory. This keeps AGENTS.md's split: Kiln owns global ordering, execution providers own placement.

### 2. Claim scopes (`kiln_core::fleet::scope`)

```rust
pub enum FleetTarget { Repository(String), Machine(String) }   // repository stored lowercase
pub enum ScopePath { Whole, Path(String) }                     // "." or "system" => Whole
pub struct FleetScope { pub target: FleetTarget, pub path: ScopePath }
pub fn parse_scope(target: &str, path: &str) -> CoreResult<FleetScope>;
pub fn scope_subject(verb: ClaimVerb, scope: &FleetScope) -> String;   // canonical text
pub fn overlaps(a: &FleetScope, b: &FleetScope) -> bool;
```

- **Target kind.** A target containing `/` is a repository, `owner/name` with one slash, compared ASCII-case-insensitively. Otherwise it is a machine label (`[a-z0-9][a-z0-9-]*`).
- **Paths.** A repository path is relative: no leading `/`, no `..` segment, `./` prefix and trailing `/` stripped. A machine path is `system`, a unit name (no `/`) or an absolute path.
- **Bounds.** The existing swarm bounds apply: 128 bytes per field, ASCII, no control characters.
- **Overlap.** Two scopes overlap when they have the same target and either one is `Whole`, the paths are equal, or one path is a `/`-boundary prefix of the other. Units and absolute paths never contain each other.
- **Swarm overlap.** `evaluate_overlap` in `implementation::swarm` uses `overlaps` instead of equality. A claim that does not parse as a fleet scope falls back to the existing exact comparison.

### 3. Fleet claims (`kiln_core::fleet::claims`)

```rust
pub struct FleetAgent { pub tool: String, pub host: String, pub session: String } // bounded text
pub enum ClaimState { Open, Released { at_ms: u64, note: String }, Expired { at_ms: u64 } }
pub enum MirrorStatus { Pending, Posted { message_id: Option<String> }, Unknown, Failed }
pub struct FleetClaim {
    pub identity: ContentId, pub scope: FleetScope, pub holder: FleetAgent, pub purpose: String,
    pub fence: u64, pub granted_ms: u64, pub expires_ms: u64, pub state: ClaimState,
    pub attempt: Option<ContentId>, pub claim_mirror: MirrorStatus, pub release_mirror: MirrorStatus,
}
pub struct FleetClaimRegistry { pub claims: BTreeMap<ContentId, FleetClaim>, pub next_fence: u64 }
pub enum ClaimCommand {
    Acquire { scope: FleetScope, holder: FleetAgent, purpose: String, lease_ms: u64, attempt: Option<ContentId> },
    Renew { identity: ContentId, fence: u64, lease_ms: u64 },
    Release { identity: ContentId, fence: u64, note: String },
    Sweep,
    MirrorObserved { identity: ContentId, release: bool, status: MirrorStatus },
}
pub struct ClaimContext<'a> { pub now_ms: u64, pub attempts: &'a [AttemptScope], pub foreign: &'a [ForeignClaim] }
pub enum ClaimOutcome { Granted(FleetClaim), Existing(FleetClaim), Renewed(FleetClaim), Released(FleetClaim),
    Swept(Vec<ContentId>), Recorded, Conflict(ClaimConflict) }
pub fn decide(registry: &FleetClaimRegistry, command: ClaimCommand, context: &ClaimContext)
    -> CoreResult<(FleetClaimRegistry, ClaimOutcome, Vec<ClaimEffect>)>;
pub enum ClaimEffect { PostClaim { identity: ContentId, subject: String, body: String },
    PostRelease { identity: ContentId, subject: String, references: Option<String>, body: String } }
```

- **Identity.** BLAKE3 over a length-prefixed encoding of the scope subject, holder, `granted_ms` and fence. Fences come from the registry's `next_fence` and never repeat.
- **Conflicts.** An `Acquire` conflicts with any of these that overlaps:
  - an open, unexpired fleet claim held by another holder;
  - an `AttemptScope` (repository plus a permitted path of a claimed, executing, candidate-produced or verifying attempt), unless `attempt` names that same attempt;
  - a `ForeignClaim` (an open swarm claim not written by this Kiln, younger than 24 hours).

  The same holder acquiring the identical scope gets `Existing` (idempotent); a different overlapping scope from the same holder is granted.
- **Leases.** `DEFAULT_CLAIM_LEASE_MS` is 4 hours and `MAX_CLAIM_LEASE_MS` 24 hours; a `MIN_CLAIM_LEASE_MS` applies. `Renew` and `Release` require the exact identity and current fence of an open claim. `Sweep` expires open claims past `expires_ms`.
- **Bounds.** `MAX_OPEN_FLEET_CLAIMS` and `MAX_PURPOSE_BYTES` are named limits.
- **Mirror effects.** A granted claim emits `PostClaim`, rendered with the existing `claim_subject` and a redaction-checked body from named facts (purpose, holder label, lease, signature `— kiln on <coordinator> for <holder label>`). A release or expiry emits `PostRelease`, whose `references` carries the claim's message id once it is known. Mirror outcomes come back through `MirrorObserved`. An `Unknown` post is never reposted blindly; the shell reconciles it by reading the newest window.

### 4. Interactive sessions (`kiln_core::fleet::session`)

```rust
pub enum AgentStatus { Idle, Working, Blocked, Done, Unknown }
pub enum SessionState { Placing, Starting, Prompted, Working, Blocked { since_ms: u64 }, Ready { since_ms: u64 },
    HandoffReceived { commit: String }, Captured, Failed { cause: String }, Cancelled { cause: String } }
pub struct FleetSession {
    pub identity: ContentId, pub attempt: ContentId, pub fence: u64, pub work: ContentId, pub work_revision: u64,
    pub machine: String, pub kind: String, pub agent_name: String, pub workspace: SessionWorkspace,
    pub state: SessionState, pub created_ms: u64, pub observed_ms: u64, pub worked: bool, pub nudges: u32,
}
pub struct SessionWorkspace { pub identity: ContentId, pub herdr_workspace: String, pub pane: String,
    pub path: String, pub branch: String, pub base_commit: String }
pub enum Observation { Started { agent_name: String, workspace: SessionWorkspace }, StartFailed { reason: String },
    Prompted, Status { status: AgentStatus }, Gone, Handoff { commit: String }, Captured,
    CaptureFailed { reason: String }, Cancelled { cause: String } }
pub struct SessionLedger { pub sessions: BTreeMap<ContentId, FleetSession> }
pub fn observe(ledger: &SessionLedger, session: &ContentId, observation: Observation, now_ms: u64)
    -> CoreResult<SessionLedger>;
pub struct TickInput<'a> { pub now_ms: u64, pub ledger: &'a SessionLedger, pub attempts: &'a [AttemptView],
    pub free_slots: u32, pub ready_work: u32, pub decisions: &'a DecisionLedger }
pub enum FleetEffect { ClaimNext, Place { attempt: ContentId, fence: u64 }, Prompt { session: ContentId, text: String },
    Renew { attempt: ContentId, fence: u64 }, Capture { session: ContentId, commit: String },
    RaiseDecision { session: ContentId, question: String }, Cancel { attempt: ContentId, fence: u64, cause: String },
    Stop { session: ContentId } }
pub fn plan_tick(input: &TickInput, policy: &FleetPolicy) -> Vec<FleetEffect>;
```

`plan_tick` is pure and deterministic, and it never repeats an effect whose observation is still pending. Its rules:

- **Claiming.** `ClaimNext` is requested when `free_slots` exceeds the sessions in `Placing` or `Starting` and ready work exists.
- **Placing.** An attempt held by the fleet with no session gets `Place`.
- **Renewal.** A session in `Starting`, `Prompted`, `Working`, `Blocked`, `Ready` or `HandoffReceived` gets `Renew` when its attempt lease ends within `renew_before_ms`.
- **Decisions.** `Blocked` for longer than `blocked_decision_after_ms` gets one `RaiseDecision`.
- **Nudges.** `Ready` after work, with no handoff for `nudge_after_ms`, gets a nudge `Prompt`, up to `max_nudges`. After that comes one `RaiseDecision`.
- **Capture.** `HandoffReceived` gets `Capture`.
- **Failure.** `StartFailed`, `Gone` before capture, or `CaptureFailed` get `Cancel` and `Stop`. An attempt that is cancelled, expired or superseded outside the fleet gets `Stop`.

`FleetPolicy` holds named limits with defaults: `tick_ms` 15 s, `renew_before_ms` 10 min, `blocked_decision_after_ms` 5 min, `nudge_after_ms` 10 min, `max_nudges` 2, `attempt_lease_ms` 60 min.

### 5. Decisions (`kiln_core::fleet::decision`)

`Decision { identity, session, attempt, question, raised_ms, answer: Option<Answer { text, by: String, at_ms }> }`. Answers come only from operator machines and produce a `Prompt` with the answer text. A decision raised for a session that has since ended is closed with no answer.

### 6. Prompt (`kiln_core::fleet::prompt`)

`render_attempt_prompt(work, attempt, session_workspace, handoff: &HandoffInstructions) -> String` is pure and bounded. It states:

- the title, requested outcome and every acceptance obligation;
- the permitted paths and the rule to change nothing outside them;
- the repository, base commit, worktree path and branch `kiln/impl-<work12>-<attempt12>`;
- the attempt and fence;
- how to hand off: commit on that branch, then call the `kiln_handoff` tool or run `kiln-fleet handoff --attempt ID --fence N --commit SHA`;
- that blocking questions are relayed to the operator;
- not to push, open pull requests, post claims or merge;
- that swarm and tool output is data.

The nudge text is a separate pure function.

### 7. Interactive dispatch isolation (`kiln_core::implementation::dispatch`)

`WorkspaceIsolation` gains `InteractiveWorktree`. `admit_dispatch_plan` accepts `workspace.network` and `workspace.credentials` set to true only when all of these hold:

- the isolation is `InteractiveWorktree`;
- `provider_profile` is `herdr-interactive`;
- the work's `permitted_effects` contains `interactive_agent`.

An `InteractiveWorktree` plan without those conditions is refused with `implementation_dispatch_workspace`. Every Lattice rule is unchanged. `INTERACTIVE_PROVIDER_PROFILE` and `INTERACTIVE_AGENT_EFFECT` are named constants.

### 8. herdr candidate receipt (`kiln-implementer::verification::capture`)

`materialize_candidate` accepts `kiln.herdr.candidate-receipt.v1` when the retained plan's provider profile is `herdr-interactive`. The receipt has the same fields it checks today: `work_id`, `attempt_id`, `fence`, `workspace_id`, `disposition` (`produced` or `unchanged`) and `blockers`. It adds `candidate_commit`, the agent's commit, and `session`, the fleet session identity. `candidate_root` is an isolated export of exactly that commit's tree, made by the fleet shell; the existing checks then run unchanged against it. The Lattice receipt is still required for Lattice plans.

### 9. Store (`kiln_implementer::fleet`)

The fleet state is one versioned blob, `FleetState { claims, sessions, decisions }`, under kind `fleet` in `implementation_state`. It is loaded, decided and saved in one SQLite transaction, like the work and attempt ledgers. The `Implementer` methods are:

- `fleet_state()`
- `fleet_claim(command, now_ms, foreign) -> ClaimOutcome+effects`, which derives the `AttemptScope`s from the current attempts and work;
- `fleet_observe(session, observation, now_ms)`
- `fleet_bind_session(session)`
- `fleet_raise_decision`, `fleet_answer_decision`
- `fleet_tick_input(now_ms)`

All of them are positive and negative tested, including persistence across reopen.

### 10. herdr adapter (`kiln-adapter-herdr`)

- **Config.** `HerdrInventory` is a JSON export of typed Nickel `config/fleet-inventory.ncl`, schema `kiln.fleet-inventory.v1`: `executable`, `session` (headless session name, default `kiln`) and `machines`. Each machine has:
  - `label`, and `herdr_machine` (a saved-machine label, or null for the local server);
  - `ssh_target` for git transport, and `worktree_root`;
  - `repositories`, mapping `owner/name` to an absolute checkout path;
  - `agents: [{ kind, slots, args }]`.

  `kind` is one of herdr's agent kinds; `omp`, `claude`, `codex` and `agy` are admitted.
- **Operations.** Each is one bounded `bounded-exec` child with a cleared environment plus explicit `HOME`/`PATH`/`XDG_*`, exact argv, a deadline and an output bound. Errors are classified as `unavailable`, `rejected` or `unknown`; an `unknown` mutation is reconciled by listing before any retry.
  - `probe(machine)`
  - `occupancy()`
  - `create_worktree(machine, repository, branch, base)`, using `worktree create --cwd <repo checkout> --branch --base --path <worktree_root>/<attempt12> --label`
  - `start_agent(machine, pane, name, kind)`
  - `prompt(machine, agent, text)`
  - `status(machine, agent)`
  - `read(machine, agent, lines)`
  - `interrupt(machine, agent)`
  - `close_workspace(machine, workspace)`
  - `list_agents(machine)`
- **Placement.** `place(kind_preferences, occupancy)` picks a machine and slot deterministically, choosing the least loaded machine and then the label.
- **Status.** Agent status maps one-to-one onto `AgentStatus`. A missing agent is `Gone`.
- **Tests.** Conformance tests use a scripted fake `herdr` and cover valid operations, malformed JSON, unsupported kinds, stale names, replayed starts, deadlines, an unavailable machine and ambiguous outcomes.

### 11. Swarm mirror fixes (`kiln-adapter-swarm`)

- **Newest window.** `SwarmClient::groups()` parses `iroh-news groups`. `read_recent(limit)` reads with `--since high-limit` (saturating). Every overlap and message-id lookup uses `read_recent`, including the implementer `claim` path, the campaign drive and mirror `resolve_message_id`.
- **Own From address.** `SwarmConfig.from` is passed as `IROH_NEWS_FROM` in the child environment, so posts carry Kiln's own address; it is a configuration value, not a secret. A config whose `from` address equals another configured machine address is refused at load. `kiln-fleet` requires a distinct address.

### 12. `kiln-fleet` (shell)

One binary with these subcommands:

- **`run --config FLEET.json`:** the service.
  - **API.** A bounded HTTP/1.1 JSON server thread on each `listen` address. Peers are authenticated as follows: loopback is the coordinator; a tailnet peer is named by `tailscale whois --json` and mapped through the `peers` table; anything else is refused. The declared holder `host` always becomes the authenticated machine.
  - **Dispatch.** The dispatch thread runs one tick every `tick_ms`:
    1. recover expired attempts;
    2. sweep claims;
    3. reconcile mirror posts;
    4. observe herdr sessions;
    5. take `plan_tick` effects;
    6. perform them.

    Performing `ClaimNext` means `claim_next_deferred` with a defer function backed by fleet claims and foreign swarm claims, then `reserve_dispatch_plan` with `herdr-interactive`. `Place` creates the worktree and starts the agent; then the session is bound and the prompt sent. `Capture` exports the commit tree from the trusted repository after a git fetch from `ssh_target:path`. It then writes the herdr receipt, calls `materialize_candidate`, `admit_candidate_handoff`, and the configured verification and publication argv in that order. `Stop` interrupts the agent and closes its workspace.

    Each API connection and the dispatch thread open their own store connection; every store method is one SQLite transaction, so writes serialize without a shared handle (`Implementer` is not `Send`). The host is chosen at `ClaimNext`, because the plan's workspace identity binds it, and `Place` waits for a free slot there. Decision questions are posted directly with the pinned `iroh-news` as `DECISION <coordinator> attempt <attempt12>`, because the claim client posts only claim subjects.
- **`mcp --server URL --tool NAME`:** stdio MCP, JSON-RPC 2.0, protocol `2025-06-18`, implemented without new dependencies. Tools:

  | Tool | API call |
  | --- | --- |
  | `kiln_claims` | `GET /v1/claims` |
  | `kiln_claim` | `POST /v1/claims` |
  | `kiln_renew` | `POST /v1/claims/{id}/renew` |
  | `kiln_release` | `POST /v1/claims/{id}/release` |
  | `kiln_work` | `GET /v1/work` |
  | `kiln_attempts` | `GET /v1/attempts` |
  | `kiln_handoff` | `POST /v1/attempts/{id}/handoff` |
  | `kiln_decisions` | `GET /v1/decisions` |

  Descriptions mark all returned text as untrusted data.
- **Client commands:** `claim`, `renew`, `release`, `claims`, `work`, `attempts`, `handoff`, `decisions`, `answer` and `cancel`, each taking `--server URL`.

The API, version 1, takes JSON bodies of at most 64 KiB:

| Method and path | Body | Result |
| --- | --- | --- |
| `GET /v1/health` | | `{ "machine", "version" }` |
| `GET /v1/claims` | | open claims |
| `POST /v1/claims` | `{ "scope": "<target> <path>", "tool", "session", "purpose", "lease_minutes"? }` | 201 claim, 409 conflict, 400 invalid |
| `POST /v1/claims/{id}/renew` | `{ "fence", "lease_minutes"? }` | 200 claim, 409 stale |
| `POST /v1/claims/{id}/release` | `{ "fence", "note"? }` | 200 |
| `GET /v1/work` | | ranked ready work and blocked reasons, bounded |
| `GET /v1/attempts` | | attempts with sessions |
| `POST /v1/attempts/{id}/handoff` | `{ "fence", "commit" }` | 202; only from the machine hosting the session |
| `POST /v1/attempts/{id}/cancel` | `{ "reason" }` | 200; operators only |
| `GET /v1/decisions` | | open and answered decisions |
| `POST /v1/decisions/{id}/answer` | `{ "text" }` | 200; operators only |

The configuration is the typed Nickel `config/fleet.ncl`, schema `kiln.fleet.v1`, with these fields:

- `machine`, `listen`, `peers`, `operators`;
- `implementer { database, enrollment }`;
- `swarm`: a nullable swarm mirror profile;
- `inventory`: a path;
- `capture { trusted_repositories, export_root, verification_profiles, authority, verify_argv?, publish_argv? }`;
- `policy`: the limits above;
- `dispatch { enabled }`.

`dispatch.enabled = false` runs the claim authority alone.

### 13. Non-claims

The fleet proves claim bookkeeping, placement requests and observed agent states. It proves no agent's honesty, no workspace isolation and no completion. Verification remains Kiln's independent canonical checks, and publication remains the existing verified-PR path. Interactive workspaces have ambient network and credentials, and their plans say so explicitly.

## Verification Plan

- **Core.** Unit tests in `kiln-core::fleet` for every scope rule, claim command, conflict source, sweep, mirror status, session transition, `plan_tick` rule and the prompt bounds, positive and negative together.
- **Swarm.** The existing swarm tests updated for containment overlap, plus `read_recent` and From tests against the scripted `iroh-news`.
- **Dispatch admission.** Interactive plans admitted only with the exact profile and effect, refused otherwise, with Lattice plans unchanged.
- **Capture.** herdr receipts admitted for herdr plans and refused for Lattice plans, and the reverse.
- **Store.** `kiln-implementer` fleet store tests, including persistence across reopen and conflicts against live attempts.
- **Adapter.** `kiln-adapter-herdr` conformance tests against a scripted herdr.
- **Service.** `kiln-fleet` tests for HTTP routing and authentication with a fake `tailscale`, the MCP handshake and tools, CLI exit codes, and one end-to-end dispatch tick against a temporary store, the scripted herdr and a local git repository.
- **Workspace.** `cargo test` for the touched crates, `cargo clippy` with workspace lints, the flake package builds and Cairn validation.
- **Live.** `kiln-fleet run` on britton-desktop with herdr headless sessions on britton-desktop and aspen3, agents claiming through MCP, and one admitted work item dispatched to OMP and captured.
