# Implementation Work Specification

## Purpose

Defines the `implementation-work` capability.

## Requirements

### Requirement: Bounded versioned implementation-work contract

r[kiln.implementation.contract] Kiln MUST admit implementation work only through
a versioned bounded contract that binds canonical identity, exact source aliases
and source versions, enrolled repository, project and actor, title, requested
outcome, reviewed acceptance obligations, dependency identities, permitted
repository-relative paths, permitted effect classes, verification profile,
attempt budget, terminal policy, and priority. Unknown schemas, unenrolled
actors or repositories, unsafe paths, unknown effects, missing acceptance,
missing budgets, and fields over their named bounds MUST fail before any version
is retained. Admission MUST NOT claim, execute, verify, publish, or complete
work, and archive readiness MUST NOT be part of the contract.

#### Scenario: Complete contract is admitted

r[kiln.implementation.contract.scenario.admit]
- GIVEN a complete bound contract with an enrolled repository and actor
- WHEN admission runs
- THEN the canonical identity and alias key MUST be deterministic
- AND the work MUST be retained as ready when acceptance and dependencies are satisfiable

#### Scenario: Malformed or unauthorized input is refused

r[kiln.implementation.contract.scenario.refused]
- GIVEN an unknown schema, an unenrolled actor or repository, an escaping path, an empty acceptance set, a zero budget, or an oversized field
- WHEN admission runs
- THEN it MUST fail before retaining a version
- AND no dispatch authority MAY be created

### Requirement: Explicit task-level source mapping

r[kiln.implementation.mapping] Linked issue and Cairn task observations MUST
resolve to one canonical work key only through an explicit source mapping. Every
mapped alias MUST appear in the admitted alias set; repeated, partial, or forged
mappings MUST be refused. Unrelated work MUST NOT merge by title similarity, and
one parent MUST retain several independent task-level work units.

#### Scenario: Linked units merge

r[kiln.implementation.mapping.scenario.linked]
- GIVEN an issue and its explicitly mapped task-level units
- WHEN both are admitted together
- THEN they MUST share one canonical key and identity

#### Scenario: Forged or unrelated mapping is refused

r[kiln.implementation.mapping.scenario.forged]
- GIVEN a mapping that omits an alias, adds an unobserved alias, or repeats one unit
- WHEN admission runs
- THEN it MUST fail
- AND an unrelated task with the same title MUST remain a separate work unit

### Requirement: Visible blocked eligibility and deterministic ranking

r[kiln.implementation.eligibility] Missing dependencies, absent acceptance
obligations, and dependency cycles MUST produce visible blocked reasons.
Blocked work MUST NOT be ranked or dispatched. Only ready work MUST be ranked,
deterministically by reviewed priority and then canonical key.

#### Scenario: Missing dependency or cycle blocks

r[kiln.implementation.eligibility.scenario.blocked]
- GIVEN a work unit with an unadmitted dependency, no acceptance obligation, or a dependency cycle
- WHEN admission or ranking runs
- THEN the unit MUST expose a blocked state with its reasons
- AND it MUST NOT appear in the ready ranking

#### Scenario: Ready ranking is stable

r[kiln.implementation.eligibility.scenario.rank]
- GIVEN several ready work units
- WHEN ranking runs repeatedly
- THEN the same identities MUST appear in the same order

### Requirement: Replay, duplicate and supersession semantics

r[kiln.implementation.replay] Duplicate or out-of-order source observations MUST
produce one retained canonical version and no dispatch by themselves. A lower
source sequence MUST be stale. An equal sequence with different content MUST be
refused as ambiguous. A higher sequence MUST supersede the retained version while
keeping the prior snapshot auditable. Replay MUST produce the same identities and
selection decisions, and an archived or deleted Cairn change MUST NOT prove work
completion.

#### Scenario: Duplicate and out-of-order deliveries

r[kiln.implementation.replay.scenario.duplicate]
- GIVEN repeated or out-of-order observations of one source alias set
- WHEN admission runs
- THEN exactly one current version MUST remain
- AND duplicate or stale observations MUST NOT dispatch work

#### Scenario: Source edit supersedes

r[kiln.implementation.replay.scenario.edit]
- GIVEN an admitted work unit and a later source revision with changed acceptance
- WHEN the later observation is admitted
- THEN it MUST supersede the prior identity
- AND the prior snapshot MUST remain in history

#### Scenario: Lifecycle state is not completion

r[kiln.implementation.replay.scenario.lifecycle]
- GIVEN an implementation-ready Cairn task whose archive readiness is false, or an archived or deleted change
- WHEN work is admitted
- THEN readiness MUST follow the explicit contract only
- AND lifecycle state MUST NOT be treated as implementation completion

### Requirement: Authenticated GitHub issue intake

r[kiln.implementation.issue_intake] Kiln MUST admit implementation intent from a
GitHub `issues` webhook only after bounding the body, validating the delivery and
event headers, and verifying the exact original bytes with the configured
HMAC-SHA256 secret. The repository and actor MUST match the reviewed policy and
enrollment. Acceptance obligations, requested outcome, permitted paths and
effects, verification profile, budget, terminal policy and priority MUST come
from operator policy, never from issue text. An opened, reopened or edited issue
MUST produce one bounded work record whose source version is the BLAKE3 of the
exact event bytes and whose sequence is the parsed issue update timestamp. A
closed issue MUST produce an explicit closed observation, never an implementation
completion. Unsupported actions and unrelated events MUST be ignored without
creating work.

#### Scenario: Authenticated issue becomes one work record

r[kiln.implementation.issue_intake.scenario.admit]
- GIVEN a valid signature, enrolled repository and enrolled actor
- WHEN an opened issue delivery is admitted
- THEN exactly one bounded work record MUST be produced
- AND its actor, repository, source alias and sequence MUST match the delivery

#### Scenario: Forged or unenrolled delivery is refused

r[kiln.implementation.issue_intake.scenario.refused]
- GIVEN an invalid signature, a mismatched repository, an unenrolled actor, or a malformed payload
- WHEN admission runs
- THEN it MUST fail before producing work

#### Scenario: Edits version and closing never completes

r[kiln.implementation.issue_intake.scenario.lifecycle]
- GIVEN an edited or closed issue delivery
- WHEN admission runs
- THEN the edit MUST produce a versioned work observation
- AND closing MUST produce an explicit closed observation, not completion

### Requirement: Exactly one current claim per work unit

r[kiln.implementation.claim] Kiln MUST admit at most one current implementation
claim per canonical work unit and MUST allow independent work units to be
claimed concurrently. A claim MUST freeze the exact work identity, work
revision, attempt ordinal and reviewed attempt budget. A second claim while a
live lease exists MUST be refused. A claim MUST NOT be inferred from a CI run,
rerun or Lattice execution identity.

#### Scenario: Racing claims yield one attempt

r[kiln.implementation.claim.scenario.race]
- GIVEN two claim requests for one work unit with a live lease
- WHEN both run against the ledger
- THEN exactly one attempt MUST become current
- AND the other MUST be refused

### Requirement: Monotonic fencing authority

r[kiln.implementation.fencing] Every accepted worker result, candidate and later
publication request MUST carry the current attempt fence. A stale or expired
fence MUST NOT register, mutate or publish a candidate, even when the old
process still runs. A replacement attempt MUST receive a strictly greater
fence.

#### Scenario: Old worker returns after replacement

r[kiln.implementation.fencing.scenario.stale]
- GIVEN an expired attempt and a higher-fence replacement
- WHEN the old worker submits a candidate with its stale fence
- THEN the submission MUST be refused
- AND the replacement MUST remain current

### Requirement: Lease expiry and replacement

r[kiln.implementation.lease] A claim MUST carry a bounded lease. Expiry MUST
block the attempt, revoke its fence and permit a replacement claim while
consuming the attempt budget. Lease renewal MUST require the current unexpired
fence.

#### Scenario: Lease expires

r[kiln.implementation.lease.scenario.expire]
- GIVEN an attempt whose lease ended
- WHEN expiry runs
- THEN the attempt MUST become blocked with a lease-expired cause
- AND a new claim MAY begin with a higher fence

### Requirement: Ambiguous dispatch reconciliation

r[kiln.implementation.dispatch] An ambiguous dispatch MUST be reconciled against
one observed provider execution before any retry. An absent recorded run MUST be
filled by the observation; an equal observation MUST be idempotent; a different
run identity MUST be refused rather than blindly dispatched again. Repeating an
attempt MUST NOT create a second execution.

#### Scenario: One execution after reconciliation

r[kiln.implementation.dispatch.scenario.reconcile]
- GIVEN a claimed attempt with an unknown dispatch acknowledgement
- WHEN the observed workflow run is reconciled twice
- THEN exactly one workflow identity MUST be retained
- AND a conflicting run MUST be refused

### Requirement: Persisted attempt states and causes

r[kiln.implementation.attempt_states] The ledger MUST persist claimed, executing,
candidate-produced, verifying, blocked, failed, cancelled and superseded states
with structured causes. Implementation failure, infrastructure failure and
unknown outcomes MUST remain distinct. Cancellation, failure and blocking MUST
revoke the fence and consume the attempt budget. The serializable ledger MUST
preserve attempt lineage and consumed budget across restart.

#### Scenario: Cancellation and restart

r[kiln.implementation.attempt_states.scenario.cancel]
- GIVEN a current attempt
- WHEN it is cancelled and the ledger is serialized and restored
- THEN the cancellation and cause MUST remain
- AND the consumed budget MUST prevent exceeding the reviewed bound

### Requirement: Supersession cannot mutate a frozen attempt

r[kiln.implementation.supersession] A newer work revision MUST follow the
configured supersession rule without changing an active attempt's frozen work
identity or revision. `AbortActive` MUST supersede and revoke the attempt;
`FinishActive` MUST allow it to finish while recording the superseding revision.
No new attempt MAY use the superseded revision.

#### Scenario: Frozen contract across a source edit

r[kiln.implementation.supersession.scenario.frozen]
- GIVEN an active attempt and a newer work revision
- WHEN supersession is applied
- THEN the attempt's stored work revision MUST remain unchanged
- AND the configured abort or finish rule MUST be visible

### Requirement: Durable atomic implementation state

r[kiln.implementation.durable_store] The implementer shell MUST persist the work
and attempt ledgers in a durable store and mutate them inside one immediate
transaction, so two schedulers cannot both claim one work unit. Every mutation
MUST be visible after restart, and a failed operation MUST leave the prior state
unchanged. The shell MUST NOT execute a worker, verify a candidate, publish, or
grant authority.

#### Scenario: Concurrent schedulers

r[kiln.implementation.durable_store.scenario.concurrent]
- GIVEN two shell instances over one store and one eligible work unit
- WHEN both attempt a claim
- THEN exactly one claim MUST succeed
- AND the other MUST observe no available claim

#### Scenario: Restart preserves dedup and budget

r[kiln.implementation.durable_store.scenario.restart]
- GIVEN admitted work, attempts and consumed budget
- WHEN the store is closed and reopened
- THEN duplicate deliveries MUST still deduplicate
- AND consumed budget MUST still bound new claims

### Requirement: Recovery reports expiry and reconciliation set

r[kiln.implementation.recovery] Recovery MUST expire ended leases, revoke their
fences, and report the active attempts that still require provider
reconciliation. It MUST NOT redispatch or invent an execution.

#### Scenario: Recovery after restart

r[kiln.implementation.recovery.scenario.restart]
- GIVEN an attempt whose lease ended while the process was stopped
- WHEN recovery runs after restart
- THEN the attempt MUST be reported expired and blocked
- AND an unacknowledged dispatch MUST be reconcilable without a second run

### Requirement: Bounded GitHub issue polling recovery

r[kiln.implementation.polling] Polling recovery MUST use a bounded provider port
and MUST normalize polled issues into the same versioned work contract as
webhook intake, including enrollment, text bounds, exact content-bound source
identities, closed observations and deterministic ordering by update sequence and
issue number. Duplicate records across pages MUST keep the highest sequence.
Exceeding the page or record bound with more records remaining MUST fail rather
than silently truncating recovery. Polling MUST NOT create execution authority or
bypass ledger admission.

#### Scenario: Pages normalize deterministically

r[kiln.implementation.polling.scenario.pages]
- GIVEN two pages containing open and closed issues
- WHEN polling recovery runs
- THEN each issue MUST produce one work or closed observation
- AND observations MUST be ordered by update sequence and number

#### Scenario: Boundaries fail closed

r[kiln.implementation.polling.scenario.bound]
- GIVEN an unenrolled actor, an oversized record, or more pages than the reviewed bound
- WHEN polling recovery runs
- THEN it MUST fail without returning a partial recovery set

### Requirement: Durable authenticated issue-delivery queue

r[kiln.implementation.delivery_queue] When configured, the GitHub webhook shell
MUST authenticate an `issues` delivery and store its exact bytes in a bounded
durable queue for a separate implementation scheduler. Repeating a delivery
identity MUST be idempotent. A full queue MUST refuse rather than evict, and an
invalid signature or non-issue event MUST NOT be queued. Absence of the queue
configuration MUST preserve prior behavior. Storing a delivery MUST NOT admit,
claim, execute or accept work.

#### Scenario: Idempotent queueing

r[kiln.implementation.delivery_queue.scenario.idempotent]
- GIVEN a configured queue and one authenticated issue delivery
- WHEN the same delivery is stored twice
- THEN exactly one queued record MUST remain
- AND the second store MUST report a duplicate

#### Scenario: Forged delivery is not queued

r[kiln.implementation.delivery_queue.scenario.forged]
- GIVEN an invalid signature or a non-issue event
- WHEN the shell handles it
- THEN it MUST fail or ignore the event without storing a record

### Requirement: Queue drain admits each delivery once

r[kiln.implementation.queue_drain] A scheduler consumer MUST drain bounded
authenticated issue deliveries into the implementer ledger by re-verifying the
signature and re-normalizing through operator policy and the versioned work
contract. A delivery MUST be removed only after admission or an explicit closed
or ignored decision. A forged or unverifiable delivery MUST fail without being
removed, so it cannot be silently lost. Repeated delivery MUST resolve to the
existing work version rather than a new one.

#### Scenario: Drain once

r[kiln.implementation.queue_drain.scenario.once]
- GIVEN one queued authenticated issue delivery
- WHEN the consumer drains it twice
- THEN exactly one work version MUST exist
- AND the queue MUST be empty after each successful drain

#### Scenario: Forged queued record

r[kiln.implementation.queue_drain.scenario.forged]
- GIVEN a queued record whose signature does not verify
- WHEN the consumer drains the queue
- THEN it MUST fail
- AND the record MUST remain queued

### Requirement: Operator drain and status commands

r[kiln.implementation.operator_cli] The implementation shell MUST provide an
operator command that drains a configured authenticated delivery queue into the
durable work ledger and a command that reports retained work and attempt counts.
The drain command MUST require absolute database, queue, secret, policy and
enrollment inputs and a positive batch bound; the status command MUST read only.
Neither command MAY claim, dispatch, verify, publish or grant authority.

#### Scenario: Drain queued work

r[kiln.implementation.operator_cli.scenario.drain]
- GIVEN a configured queue containing one authenticated issue delivery
- WHEN the operator runs drain
- THEN exactly one work record MUST be admitted
- AND the command MUST report the drained count

#### Scenario: Report durable status

r[kiln.implementation.operator_cli.scenario.status]
- GIVEN an admitted work record and no attempts
- WHEN the operator runs status
- THEN it MUST report the retained work and attempt counts
- AND it MUST NOT claim or mutate work

### Requirement: Typed implementation policy and enrollment configuration

r[kiln.implementation.nickel_config] Implementation intake policy and enrollment
MUST be authorable as typed Nickel that exports the JSON consumed by
`kiln-implementer`. The policy builder MUST bound acceptance obligations,
repository-relative paths, effect identifiers, a 1–10 attempt budget, a positive
per-attempt time, a known terminal policy, and an explicit priority. The
enrollment builder MUST require unique, non-empty repository and actor strings.
Invalid inputs MUST fail export.

#### Scenario: Valid configuration exports

r[kiln.implementation.nickel_config.scenario.export]
- GIVEN a complete policy and enrollment input
- WHEN Nickel exports them
- THEN the JSON MUST match the fields consumed by the operator command

#### Scenario: Invalid configuration is rejected

r[kiln.implementation.nickel_config.scenario.reject]
- GIVEN an absolute permitted path, an empty actor list, or an out-of-range budget
- WHEN Nickel exports the configuration
- THEN export MUST fail

### Requirement: Bounded drain loop

r[kiln.implementation.drain_loop] The operator command MUST provide a repeated
drain mode with a positive interval, an optional positive iteration bound, and
shutdown on SIGTERM or SIGINT. It MUST report the completed iteration and drained
delivery counts. A drain error MUST stop the loop. The loop MUST NOT claim,
dispatch, verify or publish work.

#### Scenario: Bounded loop

r[kiln.implementation.drain_loop.scenario.bounded]
- GIVEN an empty queue and an iteration bound of two
- WHEN the operator runs the loop
- THEN it MUST complete two iterations and report zero drained deliveries
- AND it MUST exit successfully

### Requirement: Exact resolved base participates in work identity

r[kiln.implementation.base] Admitted work MAY carry one exact resolved target/base
revision. Absence MUST remain explicit and MUST NOT be inferred from a symbolic
reference. The base MUST participate in the canonical work identity, so a base
change produces a superseding version. A claim MUST freeze the base in the
attempt, and later work edits MUST NOT mutate the frozen attempt.

#### Scenario: Base changes identity

r[kiln.implementation.base.scenario.identity]
- GIVEN otherwise equal work with and without an exact base revision
- WHEN identities are computed
- THEN the identities MUST differ

#### Scenario: Claim freezes base

r[kiln.implementation.base.scenario.freeze]
- GIVEN work with an exact base
- WHEN an attempt is claimed and the work base later changes
- THEN the attempt MUST retain the original base and work revision

### Requirement: Operator claim and cancel commands

r[kiln.implementation.operator_claim] The operator command MUST allow claiming one
current attempt with a positive lease and printing its canonical attempt JSON,
and cancelling that attempt with its exact identity and fence. A stale or missing
fence MUST fail. The commands MUST NOT dispatch, verify or publish, and a claim
MUST remain an explicit handoff to an admitted execution route.

#### Scenario: Claim and cancel round trip

r[kiln.implementation.operator_claim.scenario.roundtrip]
- GIVEN admitted eligible work
- WHEN the operator claims it and then cancels with the returned fence
- THEN the attempt MUST be persisted with claimed then cancelled state
- AND no active attempt MUST remain

#### Scenario: Stale fence cannot cancel

r[kiln.implementation.operator_claim.scenario.stale]
- GIVEN a cancelled or replaced attempt
- WHEN the operator cancels with its old fence
- THEN the command MUST fail
- AND the current attempt MUST remain unchanged

### Requirement: Cairn implementation-ready snapshot intake

r[kiln.implementation.cairn_intake] Kiln MUST admit implementation work from a
`cairn.implementation-ready.v1` snapshot only when the snapshot is complete and
its schema is supported. A ready selection MUST become one work record per stable
`task:<id>` or legacy `change:<name>` unit, bind the change snapshot digest as
the source version, carry the unit's requirement references as explicit
acceptance obligations, and use a caller-supplied monotonic sequence. Non-ready
outcomes MUST produce no work. An incomplete enumeration, incomplete snapshot,
forged or inconsistent selected unit, or already archive-ready selection MUST
fail. The intake MUST NOT claim, execute, verify or mutate Cairn lifecycle state.

#### Scenario: Ready unit becomes work

r[kiln.implementation.cairn_intake.scenario.ready]
- GIVEN a complete ready snapshot with one selected task unit
- WHEN intake runs at a monotonic sequence
- THEN one work record MUST bind the change/unit alias, digest and requirement references
- AND its acceptance MUST include both operator policy and unit requirements

#### Scenario: Non-ready outcomes produce no work

r[kiln.implementation.cairn_intake.scenario.outcome]
- GIVEN blocked, empty or completion-only Cairn outcomes
- WHEN intake runs
- THEN no work MUST be created

#### Scenario: Incomplete or forged snapshot fails

r[kiln.implementation.cairn_intake.scenario.refused]
- GIVEN an incomplete snapshot, `incomplete_limit`, a mismatched digest, an archive-ready selection or inconsistent gates
- WHEN intake runs
- THEN it MUST fail before retaining work

#### Scenario: Invalid intake policy fails

r[kiln.implementation.cairn_intake.scenario.policy]
- GIVEN an escaping path, empty acceptance, invalid budget or malformed identity
- WHEN intake runs
- THEN it MUST fail before reading the snapshot into work

### Requirement: Durable dispatch reservation

r[kiln.implementation.dispatch_reservation] Before provider transport, an
attempt MUST reserve exactly one dispatch binding over its identity, current
fence, provider profile and admitted plan identity. The reservation MUST carry a
deterministic effect identity and idempotency key. Repeating an equal reservation
MUST be idempotent; a different plan or profile for the same live attempt, a
stale fence, an expired lease, or a non-claimable phase MUST fail before
transport. The reservation MUST persist across restart. Dispatch acknowledgement
and reconciliation MUST require the reservation, accept an equal observed
workflow run idempotently, and refuse a conflicting run.

#### Scenario: Equal reservation is idempotent

r[kiln.implementation.dispatch_reservation.scenario.idempotent]
- GIVEN a claimed attempt
- WHEN the same provider plan is reserved more than once
- THEN one reservation MUST be retained with its original reservation time and identities

#### Scenario: Conflicting reservation fails

r[kiln.implementation.dispatch_reservation.scenario.conflict]
- GIVEN a claimed attempt with a reservation
- WHEN a different plan or provider profile is reserved
- THEN the reservation MUST fail before transport
- AND the original reservation MUST remain unchanged

#### Scenario: Reservation survives restart

r[kiln.implementation.dispatch_reservation.scenario.restart]
- GIVEN a reservation recorded before transport
- WHEN the durable ledger is reopened
- THEN the same reservation MUST be available
- AND execution acknowledgement MUST use it without creating a second reservation

#### Scenario: Acknowledgement requires reservation

r[kiln.implementation.dispatch_reservation.scenario.missing]
- GIVEN a claimed attempt without a reservation
- WHEN execution acknowledgement or reconciliation is requested
- THEN it MUST fail
- AND no workflow run may be recorded

### Requirement: Durable dispatch reservation

r[kiln.implementation.dispatch_reservation] Before provider transport, an
attempt MUST reserve exactly one dispatch binding over its identity, current
fence, provider profile and admitted plan identity. The reservation MUST carry a
deterministic effect identity and idempotency key. Repeating an equal reservation
MUST be idempotent; a different plan or profile for the same live attempt, a
stale fence, an expired lease, or a non-claimable phase MUST fail before
transport. The reservation MUST persist across restart. Dispatch acknowledgement
and reconciliation MUST require the reservation, accept an equal observed
workflow run idempotently, and refuse a conflicting run.

#### Scenario: Equal reservation is idempotent

r[kiln.implementation.dispatch_reservation.scenario.idempotent]
- GIVEN a claimed attempt
- WHEN the same provider plan is reserved more than once
- THEN one reservation MUST be retained with its original reservation time and identities

#### Scenario: Conflicting reservation fails

r[kiln.implementation.dispatch_reservation.scenario.conflict]
- GIVEN a claimed attempt with a reservation
- WHEN a different plan or provider profile is reserved
- THEN the reservation MUST fail before transport
- AND the original reservation MUST remain unchanged

#### Scenario: Reservation survives restart

r[kiln.implementation.dispatch_reservation.scenario.restart]
- GIVEN a reservation recorded before transport
- WHEN the durable ledger is reopened
- THEN the same reservation MUST be available
- AND execution acknowledgement MUST use it without creating a second reservation

#### Scenario: Acknowledgement requires reservation

r[kiln.implementation.dispatch_reservation.scenario.missing]
- GIVEN a claimed attempt without a reservation
- WHEN execution acknowledgement or reconciliation is requested
- THEN it MUST fail
- AND no workflow run may be recorded

### Requirement: Bounded implementation dispatch plan

r[kiln.implementation.dispatch_plan] A dispatch plan MUST bind the exact work
identity and revision, attempt identity and fence, repository, frozen base,
isolated workspace identity, provider profile, admitted paths and effects,
cancellation-only controls, positive deadline, token and invocation budgets, and
expected output identity. Admission MUST refuse any mismatch, any path or effect
the work did not admit, a deadline beyond the attempt budget, non-cancel
controls, network access, credentials, or budgets over their named bounds. The
canonical plan identity MUST be deterministic over these facts, and reservation
MUST use it.

#### Scenario: Exact plan is admitted and reserved

r[kiln.implementation.dispatch_plan.scenario.admit]
- GIVEN current work and a claimed attempt
- WHEN an exact bounded plan is admitted and reserved
- THEN the reservation MUST carry the deterministic plan identity

#### Scenario: Broader plan is refused

r[kiln.implementation.dispatch_plan.scenario.refused]
- GIVEN a plan with a different binding, broader path/effect, pause/resume control, network or credentials, an excessive deadline, or an empty/oversized budget
- WHEN admission runs
- THEN it MUST fail before reservation or transport

### Requirement: Bounded candidate handoff

r[kiln.implementation.candidate_handoff] A candidate handoff MUST bind the exact
work identity and revision, attempt identity and fence, repository, frozen base,
candidate commit and tree, complete changed paths, the canonical digest of the
work's acceptance obligations, the reviewed verification profile,
toolchain/environment identities, and retained evidence references. Every
changed path MUST be relative, unique and inside an admitted work path; candidate
and tree MUST differ; evidence identities MUST be unique and bounded. Admission
MUST refuse any mismatched work/version/fence/base/repository, transferred
acceptance digest, substituted candidate or tree, incomplete/duplicated paths,
or empty/oversized evidence. Admission MUST NOT execute a check, accept a
candidate, publish, or treat an evidence outcome as authoritative.

#### Scenario: Exact handoff is admitted and retained

r[kiln.implementation.candidate_handoff.scenario.admit]
- GIVEN an executing attempt with admitted work
- WHEN an exact complete handoff is admitted
- THEN the attempt MUST retain the handoff and become `candidate-produced`
- AND repeating the same handoff MUST be idempotent

#### Scenario: Mismatched or broader handoff is refused

r[kiln.implementation.candidate_handoff.scenario.refused]
- GIVEN a handoff with a different work/version/fence/base/repository, another acceptance digest, an out-of-scope or duplicated path, duplicate evidence, or equal candidate/tree
- WHEN admission runs
- THEN it MUST fail without mutating the attempt

### Requirement: Opt-in attempt metrics receipt

r[kiln.implementation.metrics] Kiln MAY produce a default-off metrics receipt
for one claimed attempt binding the attempt/fence/work identity and revision,
claim and terminal times, elapsed milliseconds, open/terminal phase, optional
trace references, and token and auxiliary-call observations. Missing cost data
MUST be represented as absent with a reason, never zero. Observation MUST NOT
gate admission, verification or publication. Only an explicit record request
MUST retain the first receipt on the attempt; repeated recording MUST NOT
overwrite it. Stale fences, traces over the bound, duplicate traces and
observations before the claim MUST fail.

#### Scenario: Open attempt

r[kiln.implementation.metrics.scenario.open]
- GIVEN a claimed open attempt
- WHEN metrics are observed at a later instant
- THEN elapsed time MUST span claim to observation and the receipt MUST be open
- AND absent usage MUST stay absent

#### Scenario: Terminal attempt

r[kiln.implementation.metrics.scenario.terminal]
- GIVEN a cancelled, failed or blocked attempt with a recorded end
- WHEN metrics are recorded
- THEN elapsed time MUST span claim to end and the receipt MUST be closed

#### Scenario: Default off

r[kiln.implementation.metrics.scenario.default_off]
- GIVEN an attempt with no record request
- WHEN metrics are observed
- THEN no receipt MUST be retained
- AND no admission, verification or publication decision may change

#### Scenario: First receipt is retained

r[kiln.implementation.metrics.scenario.retained]
- GIVEN an attempt with a retained metrics receipt
- WHEN recording is repeated
- THEN the original receipt MUST be returned unchanged

#### Scenario: Invalid observation is refused

r[kiln.implementation.metrics.scenario.binding]
- GIVEN a stale fence, duplicate traces, too many traces or a time before the claim
- WHEN metrics are observed
- THEN the observation MUST fail

### Requirement: Exact dispatch base

r[kiln.implementation.dispatch_base] GitHub and Cairn intake policies MAY supply
an exact resolved base revision; when supplied it MUST be carried unchanged into
the admitted work version. A dispatch plan MUST require a present base and MUST
match both the work base and the base frozen on the attempt. A plan with an
absent or different base MUST fail before reservation or transport.

#### Scenario: Policy supplies the base

r[kiln.implementation.dispatch_base.scenario.supplied]
- GIVEN an intake policy with an exact base
- WHEN an issue or Cairn unit is admitted
- THEN the work version MUST carry the same base identity

#### Scenario: Dispatch requires the exact base

r[kiln.implementation.dispatch_base.scenario.required]
- GIVEN work or an attempt without a base, or a plan with a different base
- WHEN dispatch-plan admission runs
- THEN it MUST fail before transport
