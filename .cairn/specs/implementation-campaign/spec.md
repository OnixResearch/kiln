# Implementation Campaign Specification

## Purpose

Defines the `implementation-campaign` capability.

## Requirements

### Requirement: Member-local waits keep independent work eligible

r[kiln.implementation.campaign.independent_progress] Campaign selection MUST evaluate every selected member into an admissible owner action, a hold in that member's own scope, or a wait on a dependency, the repository writer slot or observation. It MUST retain every such explanation with its member, reason, owner and effect intent. After campaign barriers (uncertain or unapplied effects, cancellation, completion, pause and bounded decision issuance), selection MUST choose the first member action in deterministic order. It MUST return a member hold or an overall wait only when no member can act. Required review, required context, stale-input replan, delivery capability and endpoint evidence MUST hold only their own member. Writer claims MUST wait while another active attempt holds the repository's writer slot. Canonical admission MUST remain the final claim, budget and lineage decision.

#### Scenario: Review wait does not stop independent work

r[kiln.implementation.campaign.independent_progress.scenario.review_wait]
- GIVEN member A whose verified candidate awaits an outstanding required review, member B that depends on A, and independent ready member C
- WHEN the campaign selects its next decision
- THEN C's coding action MUST be selected
- AND A MUST retain its review wait with the review request identity, and B MUST retain its dependency wait

#### Scenario: Member identity order cannot decide progress

r[kiln.implementation.campaign.independent_progress.scenario.permutation]
- GIVEN the review-wait fixture under every ordering of member identities
- WHEN the campaign selects its next decision
- THEN C's action MUST be selected and A's review wait MUST be retained in every ordering

#### Scenario: Member-local holds keep their scope

r[kiln.implementation.campaign.independent_progress.scenario.local_scope]
- GIVEN member A held by missing required context, stale input, semantic replan, unsupported or unobserved delivery capability, a successor milestone, or missing endpoint or review evidence
- WHEN C is independent of A
- THEN C's action MUST be selected and A's hold MUST keep its operation and owner
- AND when C depends on A, A's hold MUST be the selected decision and C MUST wait

#### Scenario: Review gate admits only exact answers and owner evidence

r[kiln.implementation.campaign.independent_progress.scenario.review_gate]
- GIVEN a single member whose candidate awaits a required review
- WHEN an answer names another candidate, policy or work, or an undeclared option, or the review expires
- THEN the answer MUST be refused and the member MUST keep waiting
- AND after an exact answer the member MUST advance only with current owner review evidence

#### Scenario: Barriers still constrain the work they govern

r[kiln.implementation.campaign.independent_progress.scenario.barriers]
- GIVEN an independent member whose action is otherwise admissible
- WHEN another attempt holds the repository's writer slot, the campaign is paused or cancelled, an effect is unapplied or uncertain, or canonical admission refuses the claim
- THEN no writer claim, dispatch or successor MAY proceed, and every retained member hold MUST remain visible

#### Scenario: The bounded driver progresses and resumes without repeats

r[kiln.implementation.campaign.independent_progress.scenario.driver]
- GIVEN the review-wait fixture in a durable campaign store
- WHEN the bounded driver runs, persists C's coding intent and dispatches it without an owner observation, and then restarts
- THEN the first run MUST claim and dispatch C once while A keeps its review wait
- AND the restarted run MUST reconcile C's retained effect without another claim or dispatch, check executing C for an ambiguous dispatch while A's hold is surfaced, and keep A's exact wait

#### Scenario: The projection shows waiting members beside the next action

r[kiln.implementation.campaign.independent_progress.scenario.projection]
- GIVEN the review-wait fixture
- WHEN the selected-campaign projection is rendered
- THEN its next action MUST be C's action with its selection basis
- AND A and B MUST show their retained selection holds and wait categories

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
