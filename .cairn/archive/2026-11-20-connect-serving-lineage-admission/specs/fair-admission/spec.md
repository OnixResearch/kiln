# Fair admission delta

## ADDED Requirements

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
