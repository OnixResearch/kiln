## Phase 1: Projection

- [x] [serial] Add the pure `cairn_workspace::agents` projection over the admitted inventory projection with the slot, pin, selection, capability, qualification and wait-reason views, and test unmanaged, unknown-with-owner, recorded, not-recorded and unpublished states over the Lattice v3 fixtures. r[kiln.cairn.workspace.agents.scenario.slots]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.slots] Report selected versus pinned revisions, blocked pins and the unpublished workflow-revision slot owned by `lattice#25`, with positive and negative cases (matching, differing, revoked, no selection, missing definition). r[kiln.cairn.workspace.agents.scenario.selection]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.selection] Keep provenance on each task-family reference and count only `live` provenance as live, with fixture and live cases. r[kiln.cairn.workspace.agents.scenario.provenance]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.provenance] Copy only allow-listed fields and this change's attempts, and prove injected sessions, workspaces, execution and grant references, model-service bindings, qualification records, preparation failures and other work's attempts never appear. r[kiln.cairn.workspace.agents.scenario.redaction]

## Phase 2: Detail, route and console

- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.redaction] Add the section to the detail (schema v3) under its view identity, read the inventory lazily for the detail route only with an independent failure boundary in the service and the exploration workspace, and test not-configured and unavailable inventory. r[kiln.cairn.workspace.agents.scenario.source]
- [x] [serial] [depends:kiln.cairn.workspace.agents.scenario.source] Render the Agents and revisions section on the Cairn changes page with distinct slot, provenance and pin words, the separate workflow slot and the source states, and add Node UI tests including ignored unknown fields. r[kiln.cairn.workspace.agents]

## Phase 3: Verification

- [x] [serial] [depends:kiln.cairn.workspace.agents] Update the operator guide, the inventory producer contract and the changelog, then run the repository gates and record the evidence. r[kiln.cairn.workspace.agents]
