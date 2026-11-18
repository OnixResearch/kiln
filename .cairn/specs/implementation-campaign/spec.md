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

### Requirement: Authenticated campaign controls through the canonical owner

r[kiln.implementation.campaign.controls] A principal MUST operate a selected campaign only through typed bounded intents (start a configured definition, continue, pause, cancel, issue a decision, resolve a decision). Intents MUST NOT carry executables, database paths, credentials, contracts or owner observations. The canonical campaign owner MUST apply each intent and its durable command record in one store transaction bound to an expected version and request ID. The receiving shell MUST authorize every mutation by a current, unexpired grant for that principal that names the campaign and command. Campaign projections, the decision inbox and start previews MUST remain read-only.

#### Scenario: Start a frozen, previewed definition

r[kiln.implementation.campaign.controls.scenario.start]
- GIVEN a configured definition with a frozen contract and an admitted delivery route
- WHEN an authorized principal reads its preview and starts it by identity
- THEN the preview MUST name the exact members, tasks, prerequisite inclusion, workflow profile, policy, repository instructions, completion goals, required checks and reviews, parent budget and delivery route
- AND the start MUST create exactly that selection, while a changed definition or an existing campaign MUST be refused without joining anything to a running campaign

#### Scenario: Offered endpoints follow the delivery route

r[kiln.implementation.campaign.controls.scenario.endpoints]
- GIVEN a definition whose member goal its delivery route does not support, such as a verified pull request on a native no-PR route
- WHEN its preview is read or a start is requested
- THEN the preview MUST mark that goal unsupported and list the goals the route offers, and the start MUST be refused

#### Scenario: Stale or unauthorized commands change nothing

r[kiln.implementation.campaign.controls.scenario.conflict]
- GIVEN a stale expected version, a changed definition or changed decision scope, a missing, narrower or expired grant, or the wrong review audience
- WHEN a command is submitted
- THEN no transition MAY occur; staleness MUST return a refreshable conflict and missing authority MUST be refused before the owner is reached

#### Scenario: Duplicate submissions reconcile one operation

r[kiln.implementation.campaign.controls.scenario.replay]
- GIVEN a command that was applied but whose response was lost
- WHEN the same principal resubmits the same intent with the same request ID, including after a restart
- THEN the owner MUST return the recorded outcome without a second transition, claim or dispatch, and the same request ID with another intent or principal MUST conflict

#### Scenario: Decisions resolve only their own current obligation

r[kiln.implementation.campaign.controls.scenario.decisions]
- GIVEN outstanding, unissued and resolved decisions
- WHEN the inbox is read and answers are submitted
- THEN the inbox MUST show each decision's question, options, recommendation, category, bound scope, scope freshness and queue state
- AND an answer MUST be refused unless it names an outstanding decision, its exact bindings and one of its options while the member's current scope still matches; it MUST NOT resolve another decision
- AND a required review MUST be answered only by a principal holding one of the member's review roles, and resolved decisions MUST remain visible

#### Scenario: Admission controls do not claim termination

r[kiln.implementation.campaign.controls.scenario.admission]
- GIVEN a campaign with retained or running work
- WHEN it is paused, continued or cancelled
- THEN pause MUST stop successor admission and decision issuance, continue MUST reopen it, and cancel MUST close it permanently
- AND no result MAY state or imply observed worker termination, provider cleanup or refunded usage

#### Scenario: Viewers read, workers are excluded, operators are narrowed

r[kiln.implementation.campaign.controls.scenario.roles]
- GIVEN viewer, worker, granted operator and ungranted operator principals
- WHEN each reads the inbox and previews and submits commands, including after a service restart and after a grant is removed from the profile
- THEN viewers MUST read with no open operations and be refused every mutation, workers MUST be refused reads and mutations, and operators MUST act only within their current grants

#### Scenario: The console offers only admitted operations

r[kiln.implementation.campaign.controls.scenario.console]
- GIVEN an inbox and previews from the service
- WHEN the console renders them and the operator submits a command
- THEN controls and answer forms MUST appear only for listed operations and actions, each submission MUST carry the displayed store version and a fresh request ID, a retry after a lost response MUST reuse that request ID, and a conflict MUST report that nothing was applied and reload the state

### Requirement: Completion-only members adopt current integrated evidence

r[kiln.implementation.campaign.adoption] A selected member whose native Cairn snapshot reports completion-only MUST NOT schedule coding work. Kiln MUST adopt existing canonical evidence only when, for every required native unit, the current canonical work's latest attempt is bound to that work, is independently verified with exactly one passing authoritative Cairn completion check, and has integration evidence from the selected delivery route for that exact work, attempt and fence; the integrated result MUST be contained in the current target and the change source MUST be unchanged since that result. The member MUST then bind the attempt whose integrated result contains every other one, at its integrated stage, and its adopted units MUST leave its remaining obligations. Otherwise the member MUST stay blocked with a reason naming the missing or stale fact.

#### Scenario: Current integrated evidence is adopted

r[kiln.implementation.campaign.adoption.scenario.adopted]
- GIVEN a freshly selected completion-only member whose required units each have verified, completion-bearing, integrated canonical evidence contained in the current target with an unchanged change source
- WHEN native observation runs
- THEN the member MUST bind the attempt whose integrated result contains the others, at its integrated stage, without a claim or a coding dispatch

#### Scenario: Missing or stale evidence stays an explicit blocker

r[kiln.implementation.campaign.adoption.scenario.blocked]
- GIVEN a completion-only member with a unit lacking current canonical work, an attempt bound to another work version, an unverified attempt, no single passing completion check, no bound integration, an integrated result the target does not contain, a change source moved after integration, or diverging integrated results
- WHEN native observation runs
- THEN the member MUST stay blocked with a reason naming that fact, and no attempt MUST be claimed or bound

### Requirement: Frozen selected campaign and retained obligations

r[kiln.implementation.campaign.scope] A campaign MUST retain a bounded versioned immutable selection revision with exact repository and whole-change or selected-task selectors, explicit prerequisite inclusion, endpoint goal and native policy/profile/instruction bindings. Every requested obligation MUST remain represented even when initially blocked, planning-only, completion-only, missing, truncated, already complete, or lifecycle-only. An incomplete/bounded source enumeration MUST NOT prove completeness. Scope, policy or profile changes MUST require explicit revision and supersession; an observation MUST NOT silently expand an active selection or attempt contract.

#### Scenario: Partial enumeration retains missing selections

r[kiln.implementation.campaign.scope.scenario.partial]
- GIVEN selected change/task identities whose Cairn observation is absent or reports incomplete_limit
- WHEN a bounded source observation is retained
- THEN each selected identity MUST remain an obligation with an explicit missing/truncated reason
- AND the campaign MUST NOT report whole-set completion

#### Scenario: New matching change does not expand frozen scope

r[kiln.implementation.campaign.scope.scenario.freeze]
- GIVEN a retained campaign selection and a later matching source observation
- WHEN the campaign reconciles its current source
- THEN the new change MUST remain outside the selection until an explicit revision
- AND the original selection and attempt bindings MUST remain auditable

### Requirement: Native workflow and owner evidence binding

r[kiln.implementation.campaign.bindings] Each member MUST bind the selected native workflow profile/version, repository development/integration instructions, checks, reviews, source/policy/subject identities and configured endpoint. Campaign progress MUST reference canonical work key/version, attempt/fence, verification, publication/integration and Cairn lifecycle owner observations; it MUST NOT create duplicate claims or infer authority from readiness, tags, process status, or context summaries. Profile-specific gates and required reviews MUST remain enforced. Unsupported prescribed delivery capability MUST block the endpoint without fallback.

#### Scenario: Required native review cannot be waived

r[kiln.implementation.campaign.bindings.scenario.review]
- GIVEN an artifact or coding result with a required review still outstanding
- WHEN a campaign action is selected
- THEN no action MAY treat the review as satisfied or authorize a successor that bypasses it

### Requirement: Evidence-aware action and durable effect reconciliation

r[kiln.implementation.campaign.controller] Pure next-action selection MUST distinguish artifact authoring/repair, coding repair, ordinary successor, infrastructure retry, unknown-outcome reconciliation, semantic replan, verification, delivery, lifecycle, wait, blocked and endpoint completion. A durable controller MUST persist decision and effect intent before owner calls, reconcile uncertain effects before dispatch, and reject stale/cancelled output by current campaign/work/attempt/fence authority. Dependency-blocked work MUST NOT prevent independent eligible work under the bounded concurrency policy. One writing worker per repository MUST be enforced through existing admission/claim owners, not a second campaign claim table.

#### Scenario: Independent work progresses while prerequisite waits

r[kiln.implementation.campaign.controller.scenario.dependencies]
- GIVEN selected prerequisite A, dependent B, and independent C, with A blocked by missing evidence
- WHEN the controller selects eligible work
- THEN B MUST remain blocked with its dependency reason
- AND C MAY progress within admitted concurrency while A remains blocked

#### Scenario: Unknown effect is reconciled after restart

r[kiln.implementation.campaign.controller.scenario.restart]
- GIVEN a persisted effect intent with an uncertain dispatch, capture, verification, publication, integration or lifecycle outcome
- WHEN the controller restarts
- THEN it MUST reconcile through the named canonical owner before any successor dispatch
- AND it MUST NOT repeat an effect already observed complete

### Requirement: Decision context and bounded human demand

r[kiln.implementation.campaign.context] Optional bounded context MUST retain stable decision/native ADR identity, exact revision, selected alternative, evidence references, supersession and projection identity bound to selection/work version, with exact supplied context retained per attempt. Decision freshness MUST remain separate from candidate/work/spec/policy/verifier/integration freshness; material context change MUST yield explicit impact/replan while unaffected evidence remains independently assessed. Preference, empirical uncertainty and policy-required review MUST be separate. Outstanding human-decision limits MUST retain unissued obligations, apply backpressure without answering/waiving gates, and retain scope-bound answers only while valid. Missing optional context MUST remain visible and nonblocking; supplied prose MUST NOT become enforced policy.

#### Scenario: Changed decision with same candidate requests impact assessment

r[kiln.implementation.campaign.context.scenario.drift]
- GIVEN a changed decision revision and unchanged candidate identity
- WHEN freshness is reconciled
- THEN the controller MUST retain candidate freshness separately and record explicit context impact/replan
- AND the changed decision MUST NOT silently rewrite an active attempt context

#### Scenario: Full decision queue retains required demand

r[kiln.implementation.campaign.context.scenario.backpressure]
- GIVEN the outstanding human-decision bound is full and a required review becomes due
- WHEN the campaign updates demand
- THEN the new obligation MUST remain visible as unissued/backpressured
- AND no gate, answer, or completion MAY be inferred

### Requirement: Endpoint-specific completion and cancellation ownership

r[kiln.implementation.campaign.completion] Campaign completion MUST require every required selected obligation to meet its configured versioned endpoint: verified candidate, verified PR, integrated change, or native lifecycle completion remain distinct. Cancellation MUST stop successor admission and reject late/stale outputs but MUST NOT claim worker cancellation, effect cleanup, or resource release without authoritative observations. Overlapping campaigns MUST attach to one canonical work/claim and define independent attribution/accounting; cancellation MUST NOT erase shared work or evidence. Parent budget lineage MUST be retained and consumed from the existing admission owner.

#### Scenario: PR endpoint is not lifecycle completion

r[kiln.implementation.campaign.completion.scenario.endpoint]
- GIVEN all selected members have verified PR evidence but native lifecycle remains incomplete
- WHEN the campaign evaluates completion
- THEN a verified-PR goal MAY complete
- AND a lifecycle-complete goal MUST remain incomplete with its lifecycle obligation visible

### Requirement: Read-only selected campaign operator projection

r[kiln.dashboard.campaign.projection] Live, CLI and offline consumers MUST expose the same bounded redacted projection of the exact selected campaign revision, goal, every selected change/task, canonical current-work/attempt/binding state, accepted candidate, integration/lifecycle milestones, pending effects, evidence/input freshness, and remaining obligations. The projection MUST distinguish prerequisite, missing-artifact, review, failed-check, worker-capability, execution-capacity, model-admission, uncertain-effect, and aggregate-budget waits; it MUST retain blocked, planning-only, completion-only, partial-enumeration, stale and missing-source members even when no member is ready. Its identity MUST bind the full final semantic payload, including canonical current-work and optional resource joins, while excluding its own identity field. Consumers MUST share identical semantic content/identity for the same admitted snapshot and safely escape untrusted text. The read MUST use the existing canonical database without schema creation, reservation, dispatch, owner discovery, or lifecycle mutation. Observation/rendering MUST NOT create a second ledger or grant successor/landing authority. Offline HTML MUST remain inert and perform no discovery/network access.

#### Scenario: Canonical current-work observation changes under an unchanged campaign row

r[kiln.dashboard.campaign.projection.scenario.current_work]
- GIVEN an unchanged stored campaign revision and store version with a changed canonical current-work revision or freshness observation
- WHEN the final read-only projection is recomputed
- THEN its semantic projection identity MUST change while the campaign-row identity remains the same
- AND the same final admitted semantic payload MUST have the same identity across live, CLI and offline consumers

#### Scenario: Blocked member remains visible without ready work

r[kiln.dashboard.campaign.projection.scenario.partial]
- GIVEN a selected member blocked by missing evidence and incomplete or stale source observations with no ready work
- WHEN the campaign projection is rendered
- THEN the selected member, source limitation, freshness, blockers and remaining obligations MUST remain visible
- AND worker/model availability MUST NOT imply task readiness or remove independent inventory errors

#### Scenario: Same snapshot identity and escaping across consumers

r[kiln.dashboard.campaign.projection.scenario.identity]
- GIVEN one admitted campaign snapshot and the same admitted optional resource snapshot
- WHEN authorized API, CLI JSON, and inert offline HTML projections are produced
- THEN each MUST contain the same semantic projection identity and selected obligations
- AND untrusted text MUST be safely escaped and bounded output MUST be rejected rather than truncated

#### Scenario: Independent unknown sources and uncertain effects remain visible

r[kiln.dashboard.campaign.projection.scenario.unknown]
- GIVEN unavailable inventory sources, unknown worker/execution/model admission, an unobserved aggregate budget, or uncertain pending effects
- WHEN the campaign is projected
- THEN each source state MUST remain explicit and independent
- AND missing sources MUST NOT imply free capacity, an unobserved budget MUST NOT imply exhaustion, and uncertain effects MUST remain visible

#### Scenario: Canonical campaign database remains read-only

r[kiln.dashboard.campaign.projection.scenario.readonly]
- GIVEN an existing canonical campaign database
- WHEN an authorized consumer reads a selected campaign
- THEN no schema or row mutation, reservation, dispatch, owner discovery, or lifecycle transition MAY occur
- AND persisted campaign/version bytes MUST remain unchanged
