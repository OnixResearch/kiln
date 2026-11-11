# Implementation Work Specification

## ADDED Requirements

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
