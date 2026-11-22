# Resource inventory delta

## ADDED Requirements

### Requirement: Workflow revision slots

r[kiln.inventory.workflow] Kiln MUST map Lattice `lattice.workflow-agent-revision-slots.v1` documents into inventory v2 workflow revision facts only through a provider adapter and a source of kind `workflow_revisions` that the inventory profile admits. Each fact MUST carry, per workflow definition, the desired, last usable and selected workflow revisions with the selection's rollout generation, kind and qualification reference, each named revision once with its compiled workflow identities and the worker definition, revision and bundle it pairs with, and pins counted per revision, status and blocked state. No guidance, context, source, session, grant or name MUST become a Kiln fact, and earlier snapshots MUST keep their encoding and identity.

#### Scenario: Captured slots map into workflow revision facts

r[kiln.inventory.workflow.scenario.mapped]
- GIVEN slots documents captured from Lattice with desired, promoted, pinned and second-revision lifecycles
- WHEN a collection maps them
- THEN each definition MUST become one fact whose slots, selection, revision bindings, worker references into the agents join source and pin counts equal the document, and no session identity or name MUST appear

#### Scenario: Malformed or contradictory slots are refused

r[kiln.inventory.workflow.scenario.refused]
- GIVEN a slots document with another schema, an unknown field, more than the byte bound, a malformed identity, selection or pin, two worker definitions, or one revision with two bindings
- WHEN the adapter reads it
- THEN it MUST refuse the document with a named reason before any fact is produced

#### Scenario: A refused slots document fails only its own source

r[kiln.inventory.workflow.scenario.isolated]
- GIVEN a collection whose workflow revision document is missing, oversized or refused by the adapter
- WHEN the collection is composed
- THEN only the workflow revision source MUST be unavailable with the refusal code and no facts, every other source MUST keep its facts, and a manifest defect MUST still refuse the collection
