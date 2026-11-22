## Phase 1: Inventory

- [x] [serial] Capture `lattice.workflow-agent-revision-slots.v1` documents from Lattice `7fb6dc47` with Lattice's own composition, fixture-port command dispatcher, session seam and `inspect` binary, and record their provenance and BLAKE3 hashes. r[kiln.inventory.workflow.scenario.mapped]
- [x] [serial] [depends:kiln.inventory.workflow.scenario.mapped] Add the provider-neutral `WorkflowRevisionSlots` facts and the `workflow_revisions` kind to `kiln_core::inventory` with admission, composition and projection, omitted when empty; test positive and negative admission and unchanged identity of earlier snapshots. r[kiln.inventory.workflow]
- [x] [serial] [depends:kiln.inventory.workflow] Add `kiln_adapter_lattice::workflow_revision_slots` with the schema probe, byte bound, strict DTOs and binding checks, and test every capture and the wrong-schema, extra-field, oversized, mismatched-worker, conflicting-revision and malformed cases. r[kiln.inventory.workflow.scenario.refused]
- [x] [serial] [depends:kiln.inventory.workflow.scenario.refused] Map the contract in the collection shell, make a refused slots document fail only its own source, admit the kind in the Nickel profile, add the source to the producer profile and pilot manifest, and test isolation and manifest refusals. r[kiln.inventory.workflow.scenario.isolated]

## Phase 2: Detail and console

- [x] [serial] [depends:kiln.inventory.workflow.scenario.isolated] Project workflow revisions in the agents section (detail schema v4) with slots, identities, paired worker comparison and counted pins, and test them over the captures. r[kiln.cairn.workspace.agents.scenario.workflow]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.workflow] Give workflow revisions their own `not_configured` and `unavailable` boundary, prove the rest of the section and detail are unchanged, and prove no session, name, guidance, context or source identity appears. r[kiln.cairn.workspace.agents.scenario.workflow_source]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.workflow_source] Render workflow revisions on the Cairn changes page, refuse v3 details, show the synthetic exploration inventory's captured workflow revisions, and add Node UI tests. r[kiln.cairn.workspace.agents]

## Phase 3: Verification

- [x] [serial] [depends:kiln.cairn.workspace.agents] Update the inventory producer contract, operator guide and changelog, run the repository gates and record the evidence. r[kiln.inventory.workflow]
