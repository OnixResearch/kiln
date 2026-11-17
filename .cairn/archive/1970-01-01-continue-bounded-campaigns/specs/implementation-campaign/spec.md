# Campaign continuation delta

## ADDED Requirements

### Requirement: Bounded campaign continuation from owner observations

r[kiln.implementation.campaign.continuation] A selected campaign MUST resume eligible work through the existing bounded driver when its blocking facts change, without a new campaign start. Wake hints MUST coalesce per campaign and MUST NOT grant effects, resources or completion. Each continuation step MUST reread canonical campaign state, MUST drive at most a configured number of batches, and MUST either drive, wait until a hint, version change, backoff deadline or bounded rescan, or stop at completion, cancellation or an exhausted failure budget. Wake demand, the last outcome, failures and backoff MUST survive restart in the campaign database without a second work queue or progress copy. A canonical capacity refusal of a claim, where another work unit wins exact admission or another writer holds the repository, MUST be a capacity wait, not a failure.

#### Scenario: Resolved review resumes the campaign

r[kiln.implementation.campaign.continuation.scenario.resume]
- GIVEN a campaign whose only member waits on a required review, after one continuation step that drove nothing
- WHEN the review is resolved through the campaign operator and the owner observes the satisfied review
- THEN the next continuation step MUST drive the member's next effect without a new campaign start

#### Scenario: Duplicate and out-of-order hints coalesce

r[kiln.implementation.campaign.continuation.scenario.coalesce]
- GIVEN several duplicate, simultaneous or out-of-order wake hints for one campaign
- WHEN continuation steps run
- THEN the hints MUST collapse into one pending demand, and no claim, dispatch, publication or integration MAY repeat
- AND a hint recorded while a batch runs MUST remain pending after that batch

#### Scenario: Bounded rescan and stale versions

r[kiln.implementation.campaign.continuation.scenario.rescan]
- GIVEN a lost notification, or a store version that moved since the last batch
- WHEN the rescan interval elapses or the version change is observed
- THEN the continuation MUST reread and drive at the current version, never replay a stale command, and never treat an empty or incomplete read as completion

#### Scenario: Waiting campaigns do not busy-loop

r[kiln.implementation.campaign.continuation.scenario.idle]
- GIVEN a campaign where no member can act, or a batch that used every operation
- WHEN continuation steps run without new hints, version changes or an elapsed rescan
- THEN the waiting campaign MUST wait without driving, and operation-limit continuation MUST stay within the configured batch bound

#### Scenario: Pause, cancellation and uncertain effects

r[kiln.implementation.campaign.continuation.scenario.pause]
- GIVEN a paused or cancelled campaign
- WHEN a continuation step runs
- THEN no new admission MAY occur, a retained uncertain effect MUST still reconcile, a paused campaign MUST wait for an operator, and a cancelled campaign MUST stop once nothing remains to reconcile
- AND observations recorded while paused MUST remain visible, and continuing admission MUST NOT enlarge the frozen selection

#### Scenario: Lifecycle-only members continue after integration evidence

r[kiln.implementation.campaign.continuation.scenario.lifecycle]
- GIVEN a member whose only remaining work is its native lifecycle step, waiting for current verifier evidence of its integrated result
- WHEN the owner records that evidence and an `integration` wake hint
- THEN the next continuation step MUST drive the lifecycle effect without a claim or a new campaign start

#### Scenario: Failures back off under a bounded budget

r[kiln.implementation.campaign.continuation.scenario.backoff]
- GIVEN driver failures, stale versions and canonical claim refusals
- WHEN their outcomes are recorded
- THEN only failures MUST count against the budget with exponential, capped backoff, the budget MUST stop the continuation when exhausted, and an operator hint MUST clear it

#### Scenario: Restart preserves continuation state

r[kiln.implementation.campaign.continuation.scenario.restart]
- GIVEN pending wake demand, a failure count and a backoff deadline
- WHEN the process restarts and reopens the campaign database
- THEN the demand, failures, backoff, selection and pending effects MUST be unchanged
