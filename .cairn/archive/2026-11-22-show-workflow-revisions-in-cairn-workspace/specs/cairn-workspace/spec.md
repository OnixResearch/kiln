# Cairn workspace delta

## MODIFIED Requirements

### Requirement: Agent revisions, capability and wait reasons in the Cairn workspace

r[kiln.cairn.workspace.agents] The Cairn change detail MUST present, from Kiln's admitted resource inventory projection and never from producer documents, each agent definition's revision facts (desired, prepared, qualified with its task-family references, evidence class and provenance, and selected for new sessions), its declared capability, each worker's pinned revision, pin and blocked state, every owner-attributed wait reason, and the workflow revisions of the inventory's workflow revision slots. Workflow revisions MUST appear as their own slot, separate from worker revisions, with their own failure boundary. The section MUST remain read-only, and an unconfigured or unavailable inventory MUST be reported with its reason without failing the detail or reading as an empty inventory.

#### Scenario: Revision lifecycle states stay distinct

r[kiln.cairn.workspace.agents.scenario.slots]
- GIVEN definitions whose revision lifecycle is unmanaged, managed with unknown slots, or managed with recorded slots, and workers with unmanaged, unknown or recorded pins
- WHEN the detail is read
- THEN each slot and pin MUST read as `unmanaged`, `unknown` with its owner, `recorded` with its value, `not_recorded` or `unpublished`, and no two of these MUST share a state or a rendering

#### Scenario: Selected and pinned revisions are shown separately

r[kiln.cairn.workspace.agents.scenario.selection]
- GIVEN a definition with a recorded selection, workers pinned to it or to another revision, and a revoked selection with blocked pins
- WHEN the detail is read
- THEN each worker MUST show its pinned revision, whether it matches the selection, and whether its pin is blocked, and no workflow revision MUST be read as a worker revision

#### Scenario: Fixture provenance is never live

r[kiln.cairn.workspace.agents.scenario.provenance]
- GIVEN task-family qualification references with `fixture` and with `live` provenance
- WHEN the detail is read and rendered
- THEN each reference MUST show its evidence class and provenance, and only a `live` reference MAY count or be rendered as live evidence

#### Scenario: Restricted context never appears

r[kiln.cairn.workspace.agents.scenario.redaction]
- GIVEN an inventory whose facts carry sessions, workspaces, execution allocations and grants, model-service bindings, full qualification records, preparation failures and attempts of other work, and workflow slots captured with session identities and names
- WHEN the detail is read and rendered
- THEN none of those values, and no workflow guidance, context or source identity, MUST appear, and a worker's attempt and fence MUST appear only when they equal an attempt of this change

#### Scenario: Inventory failure stays independent

r[kiln.cairn.workspace.agents.scenario.source]
- GIVEN no configured inventory, or an inventory read that is refused
- WHEN the detail is read
- THEN the canonical Cairn facts MUST still be returned, and the section and its workflow revisions MUST report `not_configured` or `unavailable` with the refusal code and list no definition, worker or workflow definition

#### Scenario: Workflow revisions show their identities, worker and pins

r[kiln.cairn.workspace.agents.scenario.workflow]
- GIVEN workflow revision slots captured from Lattice with desired, last usable, selected and pinned revisions pairing with worker revisions the inventory does or does not project
- WHEN the detail is read and rendered
- THEN each slot MUST read as `recorded` with the workflow revision, its compiled workflow identities and the worker revision it pairs with, `not_recorded` or `unknown`, the selection MUST show its rollout generation, kind and qualification reference, and each pin group MUST show its status, blocked state, session count, whether it matches the selection, and whether its worker revision is the one the inventory projects

#### Scenario: Missing or refused workflow revisions fail apart

r[kiln.cairn.workspace.agents.scenario.workflow_source]
- GIVEN an admitted inventory whose profile configures no workflow revision source, or whose workflow revision source is missing or unavailable because its document was refused
- WHEN the detail is read and rendered
- THEN the workflow revisions MUST read `not_configured`, or `unavailable` with the refusal code, and list no workflow definition, while the agent definitions, workers and canonical Cairn facts MUST be the same as with the workflow revisions present
