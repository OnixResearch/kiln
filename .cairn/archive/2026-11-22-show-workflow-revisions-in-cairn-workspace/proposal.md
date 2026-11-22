## Why

Kiln #84's Cairn change detail has an **Agents and revisions** section
(change `show-agent-revisions-in-cairn-workspace`). Its workflow revision slot
is fixed at `unpublished`, owner `lattice#25`, because Lattice's inventory v3
carries no workflow revisions.

Lattice #25 now publishes them. At Lattice `7fb6dc47`, `lattice agent-revision
inspect` adds `workflow_slots`, a `lattice.workflow-agent-revision-slots.v1`
document for a workflow-agent lifecycle: the desired, last usable, selected
and pinned workflow-agent revisions of one definition. Each carries the
compiled workflow's identities and the worker revision it pairs with, and pins
carry their state. Lattice projects no guidance, context, source or
per-obligation identity. Lattice #25 names Kiln #84 as the consumer: "Expose
selected/pinned workflow and worker revisions separately in Kiln workspace #84,
without leaking restricted context."

## What Changes

- Kiln reads `lattice.workflow-agent-revision-slots.v1` the way it reads
  worker inventory v3: through a Lattice adapter
  (`kiln_adapter_lattice::workflow_revision_slots`) that probes the schema,
  bounds the document, decodes a field-for-field DTO that denies unknown
  fields, validates identities and bindings, and maps it into provider-neutral
  inventory facts. A collection manifest names the documents, and the
  inventory profile admits a source of the new kind `workflow_revisions`.
- `kiln_core::inventory` gains `WorkflowRevisionSlots`: one fact per workflow
  definition, listing each named workflow revision once (compiled workflow
  identities and the worker definition, revision and bundle it pairs with),
  the desired, last usable and selected revisions (with rollout generation,
  kind and qualification reference), and pins counted per revision, status and
  blocked state. Sessions and names have no field.
- A slots document that cannot be read or is refused makes only its source
  unavailable, with the refusal code. It never fails the collection or the
  rest of the inventory.
- The Cairn change detail becomes `kiln.cairn-workspace-detail.v4`. Its
  workflow revision slot is projected from the admitted inventory: per
  workflow definition, each slot is `recorded`, `not_recorded` or `unknown`,
  shows the workflow revision and its identities, and the worker revision it
  pairs with, compared with the revision the inventory projects for that
  worker definition. Pins show status, blocked state, session count and
  whether they match the selection. No configured workflow source reads
  `not_configured`; a configured source that was not observed reads
  `unavailable` with its refusal code. The rest of the detail is unchanged.
- The Cairn changes page renders the workflow revisions as their own part of
  **Agents and revisions** and refuses v3 details.
- Fixtures: four captures from Lattice `7fb6dc47` produced with Lattice's own
  composition, command dispatcher with fixture ports, session seam and
  `inspect` binary, copied verbatim with their provenance and BLAKE3 hashes.

## Impact

- **Consumer:** operators reading a Cairn change; Lattice #25.
- **Files:** `crates/kiln-core/src/{inventory.rs,inventory_compose.rs,inventory_workflow.rs,inventory_workflow_tests.rs}`,
  `crates/kiln-adapter-lattice/src/{lib.rs,worker_inventory.rs,workflow_revision_slots.rs,workflow_revision_slots_tests.rs}`,
  `crates/kiln-web-service/src/{inventory_collection.rs,inventory_collection_workflow_tests.rs,cairn_workspace.rs,cairn.js}`,
  `crates/kiln-web-service/src/cairn_workspace/{agents.rs,agents_tests.rs,agents_workflow_tests.rs,tests.rs}`,
  `crates/kiln-web-service/examples/explore/{inventory.rs,campaign.rs}`,
  `tests/live-console-ui.test.mjs`, `config/inventory-profile.ncl`,
  `config/fixtures/inventory-producers-{positive,kind-negative}.ncl`,
  `fixtures/inventory/producers/{profile.json,pilot.collection.json,lattice-workflows/}`,
  `docs/inventory-producers.md`, `docs/operator.md`, `CHANGELOG.md` and this
  change package.
- **Verification:** adapter tests over the captures (positive, wrong schema,
  extra fields, oversized, mismatched worker binding, conflicting revisions,
  malformed identities, selections and pins, redaction), core admission
  tests, collection isolation tests, projection, detail and route tests, Node
  UI tests, and the repository gates.
- **Authority:** read-only. No selection, promotion, pin, dispatch or provider
  contact.

## Out of Scope

Live collection of slots documents from a Lattice store (Kiln #49), binding a
Cairn unit to the workflow definition its work needs (#83), per-holder
disclosure (#69/#70/#75/#76/#79), and the rest of #84. Lattice's last-usable
and adaptation facts beyond the slots document are not read.
