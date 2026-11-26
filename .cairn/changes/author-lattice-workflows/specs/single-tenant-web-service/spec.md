## ADDED Requirements

### Requirement: Lattice workflow authoring

r[kiln.sdlc.lattice_authoring]
Kiln MUST view and edit owner-admitted Lattice workflow definitions from Kiln. Forward through a configured Lattice authoring boundary. Display immutable revisions and source; submit a validated draft against the expected head. Preserve validation receipts and show stale or unsupported commands without publishing or executing automatically.

#### Scenario: Operator uses lattice workflow authoring

r[kiln.sdlc.lattice_authoring.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they edit a saved workflow from a known revision
- THEN Kiln MUST validate the draft through Lattice and save a new revision only against its expected head
