# Independent campaign progress delta

## ADDED Requirements

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
