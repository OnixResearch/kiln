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
