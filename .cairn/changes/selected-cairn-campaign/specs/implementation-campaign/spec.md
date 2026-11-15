# Selected Cairn Campaign Specification

## Purpose

Defines the selected-set implementation campaign capability while preserving canonical Kiln and Cairn authority.

## Requirements

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
