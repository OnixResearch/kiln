# Resource inventory delta

## MODIFIED Requirements

### Requirement: Explicit producer collection

r[kiln.inventory.collection] A collecting shell MUST read only the producer documents a `kiln.inventory.collection-set.v1` manifest names relative to itself, MUST compose them into one v2 snapshot by exact producer-scoped joins and never by host name, and MUST give the CLI, the live API and offline HTML the same projection identity and redaction. Composition MUST perform no discovery, network access or mutation.

#### Scenario: Same projection everywhere

r[kiln.inventory.collection.scenario.parity]
- GIVEN one collected producer snapshot
- WHEN the CLI, the live API reader and offline HTML project it
- THEN they MUST report one projection identity, and the HTML MUST contain no script or remote resource

#### Scenario: Host names never join

r[kiln.inventory.collection.scenario.hostname]
- GIVEN a host-local worker and a compute allocation on hosts with the same name
- WHEN the snapshot is composed
- THEN the worker MUST NOT reference that allocation and its execution MUST remain unknown

#### Scenario: Unsafe manifests refuse

r[kiln.inventory.collection.scenario.refused]
- GIVEN a manifest with an absolute or parent-relative document path, an unsupported producer contract or an unknown field, or an oversized or refused document of any source kind other than workflow revisions
- WHEN the shell collects it
- THEN collection MUST fail before projection; a workflow revision document is handled by `kiln.inventory.workflow.scenario.isolated` instead

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
