# Cairn workspace delta

## ADDED Requirements

### Requirement: Read-only Cairn workspace over existing observations

r[kiln.cairn.workspace] Kiln MUST provide a read-only index and detail view per Cairn change, keyed by repository identity and change name. The view MUST join canonical work, attempts, verification, publication and native lifecycle records and selected campaign members by exact identity, from one bounded read snapshot. It MUST NOT claim, dispatch, mutate lifecycle or grant authority. Worker principals MUST be refused.

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

#### Scenario: Historical evidence never reads as current

r[kiln.cairn.workspace.scenario.evidence]
- GIVEN an attempt bound to a superseded work version and an attempt bound to the current version
- WHEN their verification, publication and lifecycle records are shown
- THEN only records whose exact work identity matches the unit's current canonical work MUST read as current; others MUST read as historical

#### Scenario: Blockers name their condition, observation and owner

r[kiln.cairn.workspace.scenario.blockers]
- GIVEN canonical eligibility blocks, campaign holds, unapplied campaign effects, possibly dispatched attempts and stale member bindings
- WHEN the detail is read
- THEN each blocker MUST carry a structured condition, its supporting observation identity, the proposed resolving operation and owner, and any reason only as displayed text that is never parsed into a decision

#### Scenario: Reload and late responses keep the binding

r[kiln.cairn.workspace.scenario.reload]
- GIVEN a console URL naming a repository identity and change
- WHEN the page reloads, or a response for an earlier selection arrives after a newer one
- THEN the page MUST show exactly the URL's selection, and the late response MUST NOT overwrite it

#### Scenario: Synthetic A→B, C and completion-only D example

r[kiln.cairn.workspace.scenario.example]
- GIVEN the exploration workspace
- WHEN its Cairn changes are opened
- THEN it MUST show a prerequisite A with a dependent B, an independent C with a superseded attempt and a stale campaign binding, a completion-only D and a review wait, from the real canonical stores
