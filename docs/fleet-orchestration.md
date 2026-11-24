# Fleet orchestration

`kiln-fleet` makes Kiln the coordinator of interactive coding agents (OMP,
Claude Code, Codex, Antigravity) across several machines. Kiln is the claim
authority the agents claim through. It also dispatches admitted implementation
work to agent sessions that it starts with herdr, and it follows each session
to an ordinary candidate handoff. The service is opt-in; without
`dispatch.enabled` it runs the claim authority alone.

Kiln owns which work runs next, which claims conflict, leases and fences, and
when to ask the operator. The herdr adapter owns placement over its own
inventory. Verification and publication are Kiln's existing implementation
commands, unchanged.

## Parts

| Part | Crate | Role |
| --- | --- | --- |
| Fleet core | `kiln-core::fleet` | Pure claim scopes and containment overlap; the fleet claim registry with leases, fences, sweep, conflicts and mirror effects; the session ledger and `plan_tick`; decisions; the attempt prompt |
| Fleet store | `kiln-implementer::fleet` | `FleetState` in the implementation store, decided and saved transactionally beside the work and attempt ledgers |
| herdr adapter | `kiln-adapter-herdr` | Bounded `herdr` child processes for worktrees, agents, prompts, status and teardown, locally or through saved herdr machines; placement |
| Swarm adapter | `kiln-adapter-swarm` | Newest-window reads and Kiln's own From address (`IROH_NEWS_FROM`) |
| Service | `kiln-fleet` | The HTTP API, the stdio MCP server, client commands and the dispatch loop |

## Console page

The Kiln web console serves `?page=fleet` beside Workers and Cairn changes. The
page opens one persistent SSE stream at `/api/fleet/events`; the console writes
an initial snapshot and then a new frame whenever the aggregate fleet/work/attempt
identity changes, with heartbeats while idle. The stream closes after 45 minutes
and the page reconnects. The page also offers the same explicit commands as the
fleet CLI: acquire, renew, release, answer and cancel. The console forwards only
the reviewed routes with bounded request and response bytes; the fleet service
remains the authority for every command. Mutations require an operator principal,
and Worker principals cannot read the page's API.

The selected Cairn change also carries a read-only inline fleet panel: fleet
attempts join by exact work identity, and host-wide claims are shown when no
repository-scoped join exists. Its commands stay on the Fleet page.

The console is responsive down to phone width and is reachable through the
existing tailnet proxy; the fleet tables stack by field at narrow widths.

Enable the projection with the runtime-only `--fleet <profile>` argument (the
profile is outside the canonical service profile, so enabling observation does
not change the durable CI profile binding). Without it the page reports fleet
unavailable. The profile is a bounded local JSON file:

```json
{
  "schema": "kiln.web-fleet.v1",
  "address": "127.0.0.1:8095",
  "timeout_ms": 2000,
  "max_response_bytes": 1048576
}
```

The address must be a loopback socket address. The console never enables the
fleet service, never becomes its coordinator and never stores fleet state.

## Claim scopes

A scope is `<owner>/<repo> <path>` or `<machine> <path>`:

- A repository target is compared case-insensitively. A repository path is relative, with no `..`, and `.` names the whole repository.
- On a machine, `system` names the whole machine; otherwise the path is a unit name or an absolute path.
- Two scopes overlap when they name the same target and either names it whole, the paths are equal, or one contains the other at a `/` boundary.

The swarm mirror's deferral rule uses the same overlap. Subjects that do not parse as fleet scopes fall back to exact equality.

## Claims

A fleet claim is refused when it overlaps any of these:

- an open claim by another holder;
- the repository and permitted paths of a live implementation attempt;
- an open `CLAIM` article in the swarm younger than 24 hours that Kiln did not write.

The same holder claiming the identical scope gets its existing claim back.

Each grant has a fresh fence and a lease of 4 hours by default, between 1 minute and 24 hours. Renewal and release need the current fence. The tick sweeps expired claims.

Kiln is the swarm writer for fleet work. It posts one `CLAIM` per grant and one `RELEASE`, referencing the claim, per release or expiry, with redaction-checked bodies signed `— kiln on <coordinator> for <holder>`. An `unknown` post is reconciled from the newest window, never reposted.

## Interactive attempts

An attempt runs in an agent session only under an `ImplementationDispatchPlan` that satisfies all of these:

- `interactive_worktree` isolation;
- the `herdr-interactive` provider profile;
- network and credentials both declared true;
- work whose operator policy permits the `interactive_agent` effect.

Every other plan keeps its existing rules, including the refusal of ambient credentials. A tick does the following:

1. Recover expired attempts and sweep claims.
2. Read foreign swarm claims, observe every live session through herdr, then plan.
3. Claim the highest-ranked ready interactive work that overlaps no open claim (other work is left for Lattice), choose the least-loaded host with a free slot of a preferred agent kind, and reserve its plan, whose workspace identity binds that host and worktree path. The claim admits `claim-acquire` and the reservation admits `dispatch`, each inside its own transaction. Then take a fleet claim for its scope.
4. Place it on the chosen host: bind the `Placing` session, which admits the launch's exact `dispatch` request before the session is saved, admit that request again just before the launch effects, then create a worktree at the exact base on branch `kiln/impl-<work12>-<attempt12>`, start the agent, and send the rendered prompt with the outcome, every acceptance obligation, the permitted paths, the attempt and the fence. A denial binds or starts nothing. A host whose slots are full is retried on a later tick.
5. Renew the attempt's lease, and its fleet claim, while the session is live and while its captured candidate awaits verification, so verification that outlasts the lease does not expire the attempt and dispatch the work again.
6. Raise one decision when the session has been blocked longer than the policy window, or has stopped twice after a nudge without a handoff. The question is posted to `decision_group` as `DECISION <coordinator> attempt <attempt12>`; an operator answer is delivered as a prompt.
7. On `handoff`, which only the session's machine may send, fetch the commit and check that it descends from the base. Export exactly its tree. Admit it through `materialize_candidate` with a `kiln.herdr.candidate-receipt.v1` receipt, then run the configured verification and publication argv.
8. Cancel and stop a session whose start fails or whose agent disappears before a handoff.

## Service

```console
kiln-fleet run --config FLEET.json
kiln-fleet check-config --config FLEET.json
kiln-fleet mcp [--server URL] [--tool NAME]
kiln-fleet claim --scope '<target> <path>' --tool T --session S --purpose TEXT [--lease-minutes N]
kiln-fleet hold --scope '<target> <path>' --tool T --session S --purpose TEXT [--lease-minutes N] [--server URL] -- <command> [args...]
kiln-fleet renew --claim ID --fence N [--lease-minutes N]
kiln-fleet release --claim ID --fence N [--note TEXT]
kiln-fleet claims | work | attempts | decisions
kiln-fleet status [--format json|waybar|tmux] [--server URL]
kiln-fleet watch [--interval SECONDS] [--iterations N] [--server URL]
kiln-fleet handoff --attempt ID --fence N --commit SHA
kiln-fleet answer --decision ID --text TEXT
kiln-fleet cancel --attempt ID --reason TEXT
```

Client commands default `--server` from `KILN_FLEET_SERVER`. They exit 0 with JSON on stdout, 1 for a refusal with JSON on stderr, and 2 for usage errors. `hold` acquires an `attempt: None` claim with a 15-minute default lease (`1..=1440` minutes), runs `<command> [args...]` with inherited stdio and environment in its own process group, renews at `<= 1/3` of the lease while tracking the current `fence` and explicit `lease_minutes`, and releases the claim when the command finishes. If the claim conflicts or already exists for the same holder at start (any outcome other than a fresh `201` granted claim), `hold` prints the refusal JSON on stderr, starts nothing, and exits `1`. If a renewal is refused (`4xx`: stale fence, expired, released or swept), `hold` terminates the child process group and exits `1` without releasing. If the coordinator is unreachable during renewal (`5xx` or transport failure), `hold` logs a warning and keeps the child running while retrying until `expires_ms`, then terminates the child process group and exits `1` without releasing once the lease deadline passes. `hold` exit codes: `0` or the wrapped command's exit code (`128 + signal` if the child exits on a signal), `1` on refusal/conflict/expired-unreachable, `2` on usage error, `130` on `SIGINT`, and `143` on `SIGTERM` (after bounded child teardown and release). `check-config` validates the document only, not the paths it names. `status` prints counts as JSON by default, or one Waybar/tmux line. `watch` renders a bounded ANSI dashboard from repeated live reads and stops after `--iterations` if given. While watching, line commands `renew ID FENCE [MINUTES]`, `release ID FENCE [NOTE]`, `answer ID TEXT`, `cancel ID REASON` and `quit` call the same API; the next render shows the result.

The API serves one request per connection. Each connection and the dispatch thread open their own connection to the implementation store, whose methods are single SQLite transactions, and no store connection is held while a child process runs.

The API takes JSON bodies of at most 64 KiB. Every peer must be a configured machine: loopback is the coordinator, and a tailnet peer is named by `tailscale whois --json` and mapped through `peers`. A claim's host is always the authenticated machine, and cancellation and answers need an operator machine.

| Route | Result |
| --- | --- |
| `GET /v1/health` | machine and release version |
| `GET /v1/version` | BLAKE3 identity of the fleet, work and attempt row versions |
| `GET /v1/metrics` | Prometheus text gauges for claims, ready/blocked work, attempts and decisions |
| `GET /v1/claims` | open claims |
| `POST /v1/claims` | 201 claim, 409 conflict, 400 invalid |
| `POST /v1/claims/{id}/renew`, `/release` | 200, or 409 for a stale fence |
| `GET /v1/work`, `/v1/attempts`, `/v1/decisions` | bounded views |
| `POST /v1/attempts/{id}/handoff` | 202; only from the session's machine |
| `POST /v1/attempts/{id}/cancel`, `/v1/decisions/{id}/answer` | operators only |

The MCP server speaks newline-delimited JSON-RPC 2.0 on stdio (protocol `2025-06-18`) and offers the tools `kiln_claims`, `kiln_claim`, `kiln_renew`, `kiln_release`, `kiln_work`, `kiln_attempts`, `kiln_handoff` and `kiln_decisions`. It names the calling tool from `--tool` or from `clientInfo.name`. Every tool result is marked as untrusted data written by other agents. It also exposes read-only resources `kiln-fleet://claims`, `kiln-fleet://work`, `kiln-fleet://attempts` and `kiln-fleet://decisions`; each resource text carries the same untrusted-data boundary.

`/v1/metrics` is scrapeable on the coordinator's loopback listener. Suggested alerts: a decision open longer than 15 minutes, a claim expiring within 15 minutes, or no ready work while attempts are zero.

## Configuration

`config/fleet.ncl` types `kiln.fleet.v1`, and `config/fleet-inventory.ncl` types the herdr inventory `kiln.fleet-inventory.v1`. The service refuses a swarm `from` address equal to any configured machine's `<machine>@swarm.local`, so Kiln never posts as an agent's machine.

Kiln fetches every handed-off commit through the machine's `ssh_target`, the coordinator's too, so Kiln's account never needs to read an agent's home. Only the coordinator may omit `ssh_target`; Kiln then fetches its worktrees as a local path, and the service refuses an inventory where any other machine omits it.

## Non-claims

The fleet records claim bookkeeping, placement requests and the agent states herdr reports. It proves no agent's honesty, no workspace isolation and no completion. An interactive workspace has ambient network and credentials, and its plan says so. A captured candidate counts only after Kiln's independent verification.
