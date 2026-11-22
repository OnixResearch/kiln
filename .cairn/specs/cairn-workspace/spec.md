# Cairn Workspace Specification

## Purpose

Defines the `cairn-workspace` capability.

## Requirements

### Requirement: Read-only Cairn workspace over existing observations

r[kiln.cairn.workspace] Kiln MUST provide an index and a detail view per Cairn change, keyed by repository identity and change name. The view MUST join canonical work, attempts, verification, publication and native lifecycle records and selected campaign members by exact identity, from one bounded read snapshot. The projection and its routes MUST remain read-only and MUST NOT claim, dispatch, mutate lifecycle or grant authority. The console page MAY offer only the canonical campaign owner's decision answer and issue commands for the change's own members, as the campaign inbox admits them for the principal, and no other command. Worker principals MUST be refused.

#### Scenario: Every source resolves the same Cairn identity

r[kiln.cairn.workspace.scenario.identity]
- GIVEN canonical work aliases, campaign selections and campaign remaining obligations naming units of one change in one repository
- WHEN the index and detail are read
- THEN each unit MUST appear once under the same repository identity and change, record which sources named it, and show the exact canonical work identity, revision and readiness snapshot
- AND a campaign member's canonical work binding MUST read as current, stale, unknown or not bound by exact identity comparison

#### Scenario: Waiting members stay visible without ready coding work

r[kiln.cairn.workspace.scenario.visibility]
- GIVEN blocked, planning-only, completion-only and incompletely enumerated members and no ready coding work
- WHEN the detail is read
- THEN every member MUST remain visible with its stage, enumeration and remaining obligations

#### Scenario: Stages are separately legible

r[kiln.cairn.workspace.scenario.stages]
- GIVEN units at different points of their lifecycle
- WHEN the detail is read
- THEN readiness, execution, evidence applicability, integration and lifecycle MUST be reported as separate facets with explicit not-started, not-published and not-observed values

#### Scenario: Board lanes summarize the facets without hiding a stop

r[kiln.cairn.workspace.scenario.lanes]
- GIVEN units whose facets report a blocked or unknown readiness, a blocked, failed, cancelled, superseded or unrecognized latest attempt, a failed verification, no current work, only historical verification, current verification, observed integration and an observed lifecycle receipt
- WHEN the index and detail are read
- THEN each unit MUST carry exactly one lane: any stop MUST read as needs attention, no current work as awaiting work, and otherwise the furthest of recorded, integrated, verified, in progress or ready that its facets show, and historical evidence or an unrecognized value MUST NOT advance a unit
- AND a change MUST read as needs attention when any unit does, otherwise as its least-advanced unit's lane or awaiting work when it names no unit, with unit counts for every lane and its number of open decisions, while the five facets stay shown beside every lane

#### Scenario: Historical evidence never reads as current

r[kiln.cairn.workspace.scenario.evidence]
- GIVEN an attempt bound to a superseded work version and an attempt bound to the current version
- WHEN their verification, publication and lifecycle records are shown
- THEN only records whose exact work identity matches the unit's current canonical work MUST read as current; others MUST read as historical

#### Scenario: Blockers name their condition, observation, owner and target

r[kiln.cairn.workspace.scenario.blockers]
- GIVEN canonical eligibility blocks, campaign holds, unapplied campaign effects, possibly dispatched attempts and stale member bindings
- WHEN the detail is read
- THEN each blocker MUST carry a structured condition, whether its subject is a unit or a campaign member, the owning campaign for a member, its supporting observation identity, the proposed resolving operation and owner, and any reason only as displayed text that is never parsed into a decision
- AND blockers MUST be ordered by a fixed condition rank that puts an unreconciled dispatch outcome first, and the index MUST name each change's first blocker

#### Scenario: Dependencies and dependents stay exact

r[kiln.cairn.workspace.scenario.dependencies]
- GIVEN current work that depends on current work in the same change, on current work in another change and on work that is not current, and current work in another change that depends on this change's work
- WHEN the detail is read and its dependency graph is drawn
- THEN each unit MUST list its dependencies with their current or not-current state and its current dependents with their native identities, and the graph MUST lay them out by dependency depth, mark external and not-current nodes, and report a cycle instead of looping

#### Scenario: Scan, filter and board the change register

r[kiln.cairn.workspace.scenario.register]
- GIVEN changes in different lanes with blockers and open decisions
- WHEN the reader opens the page, types a search, chooses a lane, toggles blockers or open decisions, switches between list and board and moves with the arrow keys
- THEN rows and cards MUST be ordered deterministically with needs attention first, show their lane, unit spread, first blocker and open decisions, count the matches, and restore their view and filters from the URL without a read for every keystroke

#### Scenario: Read a change before its identifiers

r[kiln.cairn.workspace.scenario.detail]
- GIVEN a selected change with blockers, open decisions, units with attempts, dependencies and campaign members
- WHEN the reader opens it at desktop or narrow widths
- THEN its next steps, unit matrix, dependency graph and campaign members MUST precede its exact identities, each blocker MUST link its subject and owning campaign, an expanded unit MUST show its acceptance obligations, dependencies, dependents and attempt timeline, and previous, next and all-changes navigation MUST keep the register's filters

#### Scenario: Answer the change's decisions in place

r[kiln.cairn.workspace.scenario.decisions]
- GIVEN open decisions for the change's members, decisions for other changes' members in the same campaign, and principals with and without listed decision actions
- WHEN the page renders the change and an operator answers or issues a decision
- THEN answer and issue controls MUST appear only for this change's decisions whose inbox entry lists that action, each submission MUST go through the campaign command route with the inbox store version and a fresh request ID, a retry after a lost response MUST reuse that request ID, a conflict MUST report that nothing was applied and reload, and an unavailable inbox or a principal without actions MUST leave the decision read-only with a link to its campaign

#### Scenario: Reload and late responses keep the binding

r[kiln.cairn.workspace.scenario.reload]
- GIVEN a console URL naming a repository identity, change, expanded unit, view and filters
- WHEN the page reloads, or a response for an earlier selection arrives after a newer one
- THEN the page MUST show exactly the URL's selection, unit, view and filters, a missing change or unit MUST be stated and never replaced, and the late response MUST NOT overwrite it

#### Scenario: Synthetic A→B, C and completion-only D example

r[kiln.cairn.workspace.scenario.example]
- GIVEN the exploration workspace
- WHEN its Cairn changes are opened
- THEN it MUST show a prerequisite A with a dependent B, an independent C with a superseded attempt and a stale campaign binding, a completion-only D and a review wait, from the real canonical stores, with A needing attention, B ready, C in progress and D awaiting work

### Requirement: Agent revisions, capability and wait reasons in the Cairn workspace

r[kiln.cairn.workspace.agents] The Cairn change detail MUST present, from Kiln's admitted resource inventory projection and never from producer documents, each agent definition's revision facts (desired, prepared, qualified with its task-family references, evidence class and provenance, and selected for new sessions), its declared capability, each worker's pinned revision, pin and blocked state, and every owner-attributed wait reason. Workflow revisions MUST appear as their own slot, separate from worker revisions. The section MUST remain read-only, and an unconfigured or unavailable inventory MUST be reported with its reason without failing the detail or reading as an empty inventory.

#### Scenario: Revision lifecycle states stay distinct

r[kiln.cairn.workspace.agents.scenario.slots]
- GIVEN definitions whose revision lifecycle is unmanaged, managed with unknown slots, or managed with recorded slots, and workers with unmanaged, unknown or recorded pins
- WHEN the detail is read
- THEN each slot and pin MUST read as `unmanaged`, `unknown` with its owner, `recorded` with its value, `not_recorded` or `unpublished`, and no two of these MUST share a state or a rendering

#### Scenario: Selected and pinned revisions are shown separately

r[kiln.cairn.workspace.agents.scenario.selection]
- GIVEN a definition with a recorded selection, workers pinned to it or to another revision, a revoked selection with blocked pins, and no published workflow revision
- WHEN the detail is read
- THEN each worker MUST show its pinned revision, whether it matches the selection, and whether its pin is blocked, and workflow revisions MUST read as unpublished with owner `lattice#25` apart from worker revisions

#### Scenario: Fixture provenance is never live

r[kiln.cairn.workspace.agents.scenario.provenance]
- GIVEN task-family qualification references with `fixture` and with `live` provenance
- WHEN the detail is read and rendered
- THEN each reference MUST show its evidence class and provenance, and only a `live` reference MAY count or be rendered as live evidence

#### Scenario: Restricted context never appears

r[kiln.cairn.workspace.agents.scenario.redaction]
- GIVEN an inventory whose facts carry sessions, workspaces, execution allocations and grants, model-service bindings, full qualification records, preparation failures and attempts of other work
- WHEN the detail is read and rendered
- THEN none of those values MUST appear, and a worker's attempt and fence MUST appear only when they equal an attempt of this change

#### Scenario: Inventory failure stays independent

r[kiln.cairn.workspace.agents.scenario.source]
- GIVEN no configured inventory, or an inventory read that is refused
- WHEN the detail is read
- THEN the canonical Cairn facts MUST still be returned, and the section MUST report `not_configured` or `unavailable` with the refusal code and list no definition or worker
