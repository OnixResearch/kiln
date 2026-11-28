# Fleet, Factory control plane, and observation

This runbook covers Kiln's Factory control-plane status, read-only observation projections (`kiln-observe-core`, `kiln-dashboard`), stream/wait/capacity/control semantics, and the SQLite event store.

Related Factory guides:
- [Fleet orchestration](../fleet-orchestration.md): multi-machine claim authority (`kiln-fleet` / `kiln fleet ...`) and `herdr` agent dispatch.
- [Implementation runbook](../implementation-runbook.md) and [qualification](../implementation-qualification.md): durable work ledger, Lattice/OMP dispatch, and independent verification.
- [Fair admission](../fair-admission.md): opt-in tenant queues, reservations, and parent budgets.
- [Selected campaign progress and blockers](recovery-and-qualification.md#selected-campaign-progress-and-blockers): campaign controls, decision inbox, fan-out runbooks, and Cairn changes.

For the operator runbook index, see [the operator guide](../operator.md).

## Status

Status has three separate facets:

- readiness reports holds, delays, dependencies, and provider-local capacity;
- execution reports dispatch and provider observations;
- outcome reports the canonical attempt result.

Status is a rebuildable projection. It cannot mutate journal state.

## Observation projection

`kiln-observe-core` is the pure read-only projection layer for operator surfaces.
It admits a typed profile, an exact state revision, and an explicit event or log
window supplied by a shell, then returns deterministic bounded views with
`kiln.observe.view.v1` identities. It provides no transport, CLI, control, or
authority; the HTTP shell is `kiln-adapter-http-observe`, and the Iroh shell
remains a separate change. See [observation projection](../observation-projection.md)
and [HTTP observation](../http-observation.md). The iroh leg is
`kiln-adapter-iroh-observe`; see [iroh observation](../iroh-observation.md).

`kiln-dashboard` is the local operator surface over the same projection. It
admits a canonical Kiln journal document or a canonical SQLite event store,
verifies every record's event identity by recomputation, replays the record
stream canonically, and renders the bounded run, attempt, and event views as a
deterministic terminal report or a self-contained offline interactive HTML page.
The same surface is reachable as `kiln dashboard`. HTML controls search, filter,
and sort the admitted run pages and select embedded run and attempt details;
they never fetch more history or trigger CI. Inline trusted JavaScript keeps
local filter and selection state in the URL fragment across watch reloads,
while a no-JavaScript fallback retains the readable admitted report. It reads
only the files the operator names, loads no external resources, and performs no
effect. A property-based terminal rail drives the packaged binaries under a
pseudo terminal and checks the rendered report and the operator CLI against
properties derived from the staged inputs and the checked-in profiles. The same
rail has a browser specification for the HTML report and its local controls.
See [operator dashboard](../observation-dashboard.md).

Two further views, `review_friction` and `suggestion_anchors`, observe the
optional change-review workspace through an admitted review window rather than
the CI journal. Per declared authorship class (`human`, `agent`, `mixed`,
`undeclared`), `review_friction` reports:

- time from first patch set to revision-bound approval;
- patch sets before approval;
- anchored suggestions per change;
- the share of merged changes that had no suggestion, that is, changes merged
  on required CI alone;
- changes carrying at least one attested agent-submitted revision, reported as
  `agent_submitted_changes`;
- declared harness counts across those changes, reported as `agent_harnesses`.

`suggestion_anchors` ranks the most frequent suggestion anchors by path and
owning ownership rule. Use it as input for turning a recurring review comment
into a required CI check.

Authorship is self-declared by the contributor and unverified. Kiln never infers
it. Attested agent identity is a separate principal fact and does not make the
declared authorship verified or grant authority. A change counts as `undeclared` unless a patch set declares otherwise, and
pre-authorship journals replay as `undeclared`.

The views follow the existing redaction, paging and window-admission rules, and
they carry no authority. Export the window from the web service with
`GET /api/code-review/review-window?start_ms=…&end_ms=…`, then pass it to
`kiln-dashboard --review <path>` or to an observation shell snapshot. See
[observation projection](../observation-projection.md#review-friction-and-authorship).

## Logs and follow

A cursor binds one exact stream and byte offset. A read reports gaps when retained data no longer contains that offset.

A completed follow reports the final cursor and terminal stream state. Truncation does not change an attempt outcome.

## Wait

A wait request binds an exact selection and one named predicate. A client deadline changes only the client result.

The supported predicates are `all_terminal`, `any_terminal`, and `all_successful`.

## Capacity and readiness

Global tenant fairness runs before provider-local capacity admission. Capacity classes do not select workers or calculate tenant shares.

The shell supplies time and capacity observations. The core does not read a clock or poll a provider.

## Controls

An admission hold stops new dispatches in its scope. It does not pause a running attempt.

Cancellation, pause, and resume require an exact active attempt. They also require the provider capability.

Kiln does not support arbitrary process input as a standard control.

## SQLite event store

`kiln-adapter-sqlite` pins `rusqlite 0.40.2`, `libsqlite3-sys 0.38.2`, and bundled SQLite `3.53.2`.

The profile uses immediate transactions, WAL, and `FULL` synchronization. Each transaction commits accepted events, stream revision, and outbox records together.

Before retention, write a snapshot at the exact retention boundary. Replay rejects a retained history without that snapshot.

Use `backup_to` only with a new destination. The adapter runs SQLite integrity checks and records the backup BLAKE3 identity.

Schema changes require an admitted migration edge. Unknown or partially applied versions stop service admission without destructive repair.

