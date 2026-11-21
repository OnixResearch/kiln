## Context

`kiln_adapter_lattice::worker::run` is the shell behind
`kiln-lattice-dispatch --worker-request`. It checks the frozen request against
the admitted plan and work, reserves the plan, seals the request bytes, gates
admission, persists the transport intent (`begin_dispatch_transport`), runs
`lattice implementation-attempt run` (or `recover` after a prior intent) under
Bounded Exec with a cleared environment, and reconciles any exit-zero
observation as the attempt's single execution. Any other exit is retained as an
unknown outcome.

The campaign (`kiln-web-service::implementation_campaign::binding::bind`)
writes a seed, runs `lattice implementation-attempt prepare` once, and records
the prepared worker profile as the plan's `native_request_profile`.

Lattice `db64cd65` publishes the coordinator contract
(`docs/candidate-worker.md`, "Coordinator transport"):

- `dispatch_plan` is an optional request field (64 lowercase hex), bound by
  the request bytes and echoed by every observation. It is not part of the
  profile revision, so adding it does not change `worker_profile`.
- `network` admits only `false`.
- Refusals print one `lattice.implementation-attempt-refusal.v1` record with
  `reason`, `work_id`, `attempt_id`, `fence` and `dispatched`, and exit
  non-zero. `dispatched` is whether a worker effect of the refused attempt may
  have started:

  | reason | dispatched |
  |---|---|
  | `stale_fence` | `false` |
  | `crossed_attempt` | `false` |
  | `substituted_request` | `true` |
  | `profile_revision_superseded` | `true` with a retained intent, else `false` |
  | `revision_not_admitted` | `false` |
  | `revision_blocked` | `false` |
  | `export_revoked` | `true` |

- `recover` without an intent closes the attempt and prints
  `status:"not_started_closed_by_recovery"`, `result:null`, exit zero. The
  attempt can never run afterwards.
- Standalone `run` resolves the broker credential only from its own process
  environment, by the exact `env` key the request names.

## Goals and Success Contract

- The exact request bytes bind the reserved dispatch plan; a mismatch is
  refused before transport intent.
- The local fixture and runbook describe what Lattice actually admits.
- The Nickel transport contract matches the campaign's v2 requirement.
- Credential-bearing model routes work without exposing any other ambient
  variable.
- A refusal Lattice proves happened before any effect ends the attempt
  without an unknown outcome; every other refusal stays unknown.
- A closed-before-execution recovery is recorded as no execution.

## Decisions

### Decision: Prepare twice, bind the plan in the second request

**Choice:** `binding::bind` prepares the seed to a `.profile.json` probe,
sets `native_request_profile` from it, computes the plan digest with
`admit_dispatch_plan`, writes a `.planned-seed.json` with `dispatch_plan`, and
prepares that seed to the final request. It then requires the final request's
worker profile to equal the probe's and its `dispatch_plan` to equal the
digest. `validate_retained` also checks `dispatch_plan`.

**Rationale:** The plan digest frames `native_request_profile`, which only
Lattice's preparation computes, and the request must carry the digest. The
profile revision does not frame `dispatch_plan`, so a second preparation
yields the same profile. Each file is written once (`write_private_idempotently`
and Lattice's create-new output), so a crash between steps resumes from the
retained probe rather than preparing a different request.

**Alternatives considered:** Computing the profile revision in Kiln would
duplicate Lattice's framing and drift with it.

### Decision: The dispatch plan check sits after reservation

**Choice:** `worker::run` requires `request.dispatch_plan` to equal
`reservation.plan` right after `reserve_dispatch_plan` and the request seal,
before admission and `begin_dispatch_transport`. A successful or closed
observation must echo the same `dispatch_plan`.

**Rationale:** The reservation is the authoritative plan digest; failing
before the transport intent leaves no unknown outcome.

### Decision: One credential variable, decided by a pure function

**Choice:** `worker_settlement::credential_variable(&request)` returns
`Ok(None)` for an absent or `null` credential, `Ok(Some(key))` for
`{provider:"env", key, field:null|absent}` with a key matching
`[A-Z][A-Z0-9_]{0,127}`, and a typed `CredentialRefusal` otherwise:
`Malformed` (shape or field), `UnsupportedProvider`, `MalformedKey`, or
`ReservedName` for process-control names (`PATH`, `HOME`, `LD_*`,
`LATTICE_*` and the rest of a named list). The shell resolves the value only
for a fresh `run`, before `begin_dispatch_transport`; an absent or empty value
is `CredentialRefusal::Missing`. The launch uses `EnvironmentMode::Clear` with
that single entry. `recover` and `cancel` keep an empty environment because
neither starts a worker.

**Rationale:** The request is already admitted and sealed, so its credential
reference is the only authority for naming a variable. Refusing before the
transport intent keeps a missing credential from becoming an unknown outcome.
Reserved names would change how the Lattice process itself runs.

**Alternatives considered:** Passing the variable to every Lattice command
would expose the secret to commands that never resolve it.

### Decision: Settle only proven before-effect refusals

**Choice:** `worker_settlement::classify_refusal(envelope, stdout)` is pure.
The shell calls it only for a non-zero exit (`Disposition::ExitFailed`); it
parses the complete stdout as one refusal record. It returns
`BeforeEffect { reason }` only when the schema is exact, the `work_id`,
`attempt_id` and `fence` equal the request envelope, `dispatched` is `false`
and the reason is one Lattice's table allows as not dispatched
(`stale_fence`, `crossed_attempt`, `profile_revision_superseded`,
`revision_not_admitted`, `revision_blocked`). Everything else returns
`Retain`, including an unknown reason, a crossed attempt, `dispatched:true`,
`dispatched:false` for a reason the table marks as always dispatched, and
more than one record. Timeouts, cancellation and output overflow are never
parsed.

`BeforeEffect` calls the new `Implementer::settle_dispatch_without_execution`,
which applies `AttemptLedger::settle_dispatch_without_execution`: the attempt
must be current at its fence, have its reservation and transport intent and
no recorded workflow; it becomes `Failed` with `FailureClass::Infrastructure`
and cause `lattice_refused_before_effect:<reason>` and its transport intent is
removed. The dispatch reports the settlement as an error naming the reason.

**Rationale:** Lattice is the only witness of whether its state root started a
worker, and it states that per record. Unknown or contradictory records must
fail closed. Removing the intent records that no transport is outstanding,
and the terminal phase prevents any further dispatch of that attempt.

### Decision: Closed-before-execution is no execution

**Choice:** An exit-zero observation with status
`not_started_closed_by_recovery`, `result:null`, the same envelope and echoed
`dispatch_plan` settles the attempt through the same transition with cause
`lattice_closed_before_execution`, instead of `reconcile_dispatch`.

**Rationale:** Lattice exclusively created the closure marker, so the attempt
cannot execute. Recording it as an execution would bind a workflow that never
ran.

### Decision: Transport v2 only in Nickel

**Choice:** `Transport.schema` admits exactly
`kiln.lattice-implementation-cli.v2`. `implementation-worker-positive.ncl`
exports a v2 transport; `negative-implementation-worker-transport-v1.ncl`
must be rejected.

## Risks / Trade-offs

- A refused attempt consumes its attempt budget. That is deliberate: Kiln
  must resolve the conflicting Lattice state before claiming again.
- `profile_revision_superseded` with `dispatched:false` is settled; with
  `dispatched:true` it is retained, as Lattice requires.
- `not_dispatched_binding_rejected` and `not_dispatched_revision_blocked`
  observations are still reconciled as executions. They need provider
  compute ports that the standalone CLI does not have; they are out of scope.
- The admission ledger's owner reservations are not released by a
  settlement; owner reconciliation still owns them.
