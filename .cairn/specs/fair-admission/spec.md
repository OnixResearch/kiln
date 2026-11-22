# Fair Admission Specification

## Purpose

Defines the `fair-admission` capability.

## Requirements

### Requirement: Bounded opt-in admission
r[kiln.admission.bounded]
Kiln MUST filter existing work eligibility before existing fairness ranking, bound ready queues and lineage budgets, and preserve exact owner operation/generation debits and uncertainty. Transfers MUST NOT count as consumption. Shared deployment allocations MUST be charged once while worker requests retain separate charges.

#### Scenario: Competing child burst
- GIVEN two comparable campaigns and an owner-confirmed reservation
- WHEN one campaign queues children
- THEN immediate held debits and aggregate parent ceilings MUST prevent quota multiplication

### Requirement: Durable effect gating
r[kiln.admission.durable]
Explicit opt-in admission MUST revalidate current work, claim/fence, policy, deadline and provider generation after grant and persist before dispatch. Unknown effects MUST retain accounting across restart; waiting MUST NOT reset consumed attempts or authorize fallback.

#### Scenario: Possible acceptance
- GIVEN a persisted dispatch intent and lost response
- WHEN the store reopens
- THEN reconciliation MUST retain the debit and MUST NOT blindly repeat dispatch or falsely refund it

### Requirement: Synthetic review-queue wait measurement

r[kiln.admission.queue_wait] Kiln MUST keep an off-by-default synthetic fixture that runs a frozen, typed load of exact source candidates through the review queue's ordinary enqueue and dequeue commands and advance loop, under a declared finite load shape and a synthetic clock, in both serial and speculative queue modes. Its receipt MUST record, for every candidate, the exact source commit, every exact integration candidate and CI run, the arrival, enqueue, start, CI request, CI dispatch and terminal times observed at tick boundaries, and exactly one named outcome: submitted, waiting, rejected, expired, failed, unknown or cancelled. Review-queue wait, merge-queue wait, ready-work admission wait and inference backpressure MUST be separate typed series. A series the fixture did not exercise MUST be reported as not measured, with its reason. A receipt MUST be refused when a series is labelled as another series or when its summary does not match its candidates. The fixture MUST NOT change default admission, queue or budget policy, and findings from outside Kiln MUST NOT be used as pass or fail thresholds.

#### Scenario: Frozen load records every outcome

r[kiln.admission.queue_wait.scenario.outcomes]
- GIVEN the frozen load with passing, failing and unknown CI, a submitter deadline and an arrival beyond the queue bound
- WHEN the fixture runs it in each queue mode
- THEN every candidate MUST carry its exact identities, observed times and one named outcome. A candidate that never reached a terminal state MUST be reported as waiting or unknown, never as submitted

#### Scenario: Series stay separate

r[kiln.admission.queue_wait.scenario.series]
- GIVEN a receipt from a fixture that has no external merge queue, ready-work admission or inference backpressure in its path
- WHEN the receipt is read back
- THEN only review-queue wait MUST carry numbers, the other three series MUST be not measured with a reason, and a receipt that relabels a series or whose summary differs from its candidates MUST be refused

#### Scenario: Malformed load is refused

r[kiln.admission.queue_wait.scenario.load]
- GIVEN a load whose schema, bounds, candidate identities, modes or windows are invalid
- WHEN it is exported from Nickel or admitted by the fixture
- THEN it MUST be refused before any queue command runs

### Requirement: Serving backpressure from owner snapshots

r[kiln.admission.serving] Under an explicit serving policy, Kiln MUST gate worker-request work on a fresh, monotonic, provider-neutral serving-owner snapshot of the owner's aggregate scope before claim and before a tentative request, and MUST report stale, missing, incompatible, not-ready and overloaded snapshots as distinct outcomes. A snapshot MUST NOT reserve capacity, replace the owner grant, refund an operation or reset an attempt.

#### Scenario: Two callers see the last slot

r[kiln.admission.serving.scenario.last_slot]
- GIVEN a fresh owner snapshot with one remaining slot and two eligible worker requests
- WHEN both request through the owner
- THEN only the owner's grant MUST allow dispatch, the rejected request MUST be refunded only by owner rejection, and a saturated snapshot MUST hold work as overloaded without a claim or request

#### Scenario: Stale or untrusted snapshot

r[kiln.admission.serving.scenario.stale]
- GIVEN a snapshot that is too old, out of order, host-probed, source-stale or for other bounds
- WHEN selection or a request runs
- THEN the work MUST remain blocked with a stale, incomplete or incompatible reason while compatible independent work progresses

### Requirement: Recursive budget lineage

r[kiln.admission.lineage] Every descendant MUST stay inside its root's finite aggregate quota. Revocation, parent cancellation and ancestor expiry MUST block new descendant claims, requests, transfers and dispatch without refunding outstanding or unknown operations, and a lineage summary MUST keep available, tentative, held, unknown, settled and rejected usage separate and fail closed unless they conserve the root quota.

#### Scenario: Two-level recursive fixture

r[kiln.admission.lineage.scenario.recursive]
- GIVEN the Nickel-exported two-level profile charging root coordination, model subcalls and nested child executions
- WHEN children race for the final allowance, call identities, workers, depth and batching change, receipts repeat and the store reopens
- THEN no quota MUST be minted, duplicate receipts MUST NOT double-charge, outstanding child operations MUST survive reopen, stale-incarnation completions MUST be refused and unknown usage MUST stay unknown

#### Scenario: Cancelled or revoked lineage

r[kiln.admission.lineage.scenario.blocked]
- GIVEN a parent attempt that is cancelled, expires or whose budget is revoked
- WHEN descendants enqueue, select, request, transfer or dispatch
- THEN each MUST be refused or held with an explicit reason, while an unrelated campaign still progresses

### Requirement: Typed admission profiles

r[kiln.admission.profiles] Serving policy and recursive lineage profiles MUST be authored through typed Nickel contracts whose committed generated exports are checked for staleness and admitted again by Rust, and invalid profiles MUST fail to export.

#### Scenario: Profiles round-trip

r[kiln.admission.profiles.scenario.exports]
- GIVEN the positive and negative serving-policy and lineage fixtures
- WHEN the fixture check and Rust tests run
- THEN positives MUST match their committed exports and admit in Rust, and negatives MUST fail to export
