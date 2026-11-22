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

### Requirement: Registered host-local owner

r[kiln.admission.host_owner] For one explicitly registered host-local owner, Kiln MUST derive the admission policy, finite root account, queue rows, usage and eligibility observations, per-claim reservation and owner reply from the reviewed registration through the existing ledger transitions and owner port, with no second queue, scheduler or capacity ledger. The owner MUST grant only its declared capacity against the reservations the ledger retains for it, MUST NOT invite more eligible work than its free capacity, and MUST NOT report remote, provider or serving capacity. Unregistered repositories or work classes, a crossed claim, a stale or other-generation observation, a second reservation for the same work and a request beyond capacity MUST be refused before any grant, and an interrupted tentative request MUST be resolved from its own record. Replay and restart MUST NOT duplicate a reservation or reset a budget. A reservation MUST be settled only after the holding attempt is observed terminal and resolved; one that provably never reached transport MUST return without consumption, and once transport may have started the worker MUST be observed stopped, so supersession or lease expiry alone MUST NOT free the slot.

#### Scenario: Sandbox reservation passes the dispatch gate

r[kiln.admission.host_owner.scenario.reserve]
- GIVEN a registration admitting `onixresearch/implementer-sandbox` and one current ready work in its registered class
- WHEN the operator admits, observes, claims and reserves through the host-owner commands
- THEN the owner MUST commit exactly one reservation for that claim
- AND the existing pre-transport gate MUST admit dispatch intent with the owner's fresh observation

#### Scenario: Refused requests make no grant

r[kiln.admission.host_owner.scenario.refused]
- GIVEN an unregistered repository, an observation older than its window, a host whose capacity another reservation holds, or a work that already holds a reservation
- WHEN a reservation is requested
- THEN it MUST be refused with an explicit reason
- AND no new reservation or debit MUST be retained

#### Scenario: Restart and replay keep one reservation and the budget

r[kiln.admission.host_owner.scenario.replay]
- GIVEN a committed host reservation and a store that is reopened
- WHEN admit, observe and reserve run again, or a larger quota is registered
- THEN the retained operation MUST be returned without a new request, the account and quota MUST be unchanged, and the larger quota MUST be refused
- AND release MUST be refused until the attempt is observed terminal, then return a never-dispatched slot without consumption exactly once
- AND a holder whose transport may have started MUST keep its slot until a termination is reconciled or a candidate or the worker observation from Lattice's complete native result is retained, while a refusal before any effect or a crash before the transport start MUST be released once the attempt is terminal
- AND an attempt claimed from a host-owner queue row MUST be bound, in the claim's transaction, to require a recorded transport start, so any workflow acknowledgement (`start`, `reconcile` or another workflow write) MUST be refused until the transport start is recorded, while attempts not claimed from a host-owner queue row MUST be unaffected
- AND a dispatch `--recover` without a persisted transport start MUST be refused before any write, and under a policy rooting a host-owner campaign any claim that bypasses admission MUST be refused in the claim's transaction, while stores without a host-owner root MUST keep every claim path unchanged

### Requirement: Typed host-owner registration

r[kiln.admission.host_owner_profile] The host-owner registration MUST be authored through a typed Nickel contract whose committed generated export is checked for staleness and admitted again by Rust with the same capacity, quota, window and uniqueness bounds, and an invalid registration MUST fail to export.

#### Scenario: Registration round-trips

r[kiln.admission.host_owner_profile.scenario.exports]
- GIVEN the positive implementer-sandbox registration and negative capacity, window, repository, class, quota and empty fixtures
- WHEN the fixture check and Rust tests run
- THEN the positive MUST match its committed export and admit in Rust, and every negative MUST fail to export and be refused by Rust re-admission

### Requirement: Admission-governed operator claim

r[kiln.admission.operator_claim] When an explicit admission policy is retained, the operator claim MUST select and bind the queue row through the admission claim transition under an explicit authority, so the queue claim and attempt/fence agree. Without a policy the existing claim MUST be unchanged.

#### Scenario: Governed claim binds the queue row

r[kiln.admission.operator_claim.scenario.governed]
- GIVEN a configured policy and one queued, freshly observed work
- WHEN the operator claims without an authority and then with a local-operator authority
- THEN the first MUST be refused without a claim
- AND the second MUST return the attempt whose identity and fence the queue row's claim records

### Requirement: Budget lineage sent to Lattice

r[kiln.admission.lattice_lineage] When `kiln-lattice-dispatch` builds the request for a fresh dispatch, it MUST derive Lattice's `lattice.kiln-budget-lineage.v1` from the admission ledger. The root MUST be the campaign root and the account MUST be the queue row's account, with that account's parent and depth. The remainder MUST be the per-dimension sum of the attempt's owner-committed reservations for the same work, attempt and fence, with dimension names unchanged so that Lattice lists every dimension it cannot charge as unaccounted. Every dimension the account tracks MUST have a positive remainder, and each amount MUST fit the work budget and the root allocation. The lineage MUST be emitted only when the attempt holds a committed reservation; without one, the request MUST keep its bytes and identities. When the lineage cannot be derived consistently, the dispatch MUST be refused with a typed reason before the owner-grant gate and any transport start, and a guessed lineage MUST NOT be sent.

#### Scenario: The worker route carries the lineage

r[kiln.admission.lattice_lineage.scenario.carried]
- GIVEN an attempt whose host-owner or nested-account reservation is committed
- WHEN the `--worker-request` route builds its request
- THEN the request MUST be the admitted bytes with one appended `lineage` member matching the committed golden bytes
- AND Lattice's published schema MUST accept that lineage, with the dimensions it cannot charge listed as unaccounted
- AND the frozen request MUST hold those bytes before the owner-grant gate, and recovery MUST reuse only a frozen request that is the admitted bytes, or the admitted bytes plus one well-formed lineage

#### Scenario: The exchange route checks but never carries it

r[kiln.admission.lattice_lineage.scenario.exchange]
- GIVEN the same ledger and a `--dispatch-plan` workflow-exchange dispatch
- WHEN the route builds its request
- THEN it MUST derive the lineage with the same function and keep the `lattice.workflow-request` bytes unchanged when the lineage is consistent
- AND it MUST refuse with the same typed reason when the lineage is inconsistent

#### Scenario: Inconsistent lineage is refused before transport

r[kiln.admission.lattice_lineage.scenario.refused]
- GIVEN a tentative or unknown reservation, a reservation on another account, a queue row claimed by another attempt, a grant beyond the work budget, a tracked dimension the attempt does not hold, a revoked or expired lineage, an uppercase identity, or an admitted request that already names a lineage
- WHEN a dispatch route builds its request
- THEN it MUST return the matching typed `LineageRefusal` and start nothing
