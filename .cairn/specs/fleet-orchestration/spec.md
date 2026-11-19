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
