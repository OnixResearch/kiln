## Phase 1: Core composition

- [x] [serial] Add pure provider-neutral fragment composition with exact joins, serving pool fill and ambiguity refusals to `kiln_core::inventory`. r[kiln.inventory.collection]

## Phase 2: Producer adapters

- [x] [serial] [depends:kiln.inventory.collection] Map Lattice worker inventory profiles and instances into definitions and workers with explicit unknowns and conservative lifecycles. r[kiln.inventory.producers.scenario.lattice]
- [x] [serial] [depends:kiln.inventory.collection] Map compute allocation observations into allocation facts and authority-keyed pools with counted-once accounting and replay refusal. r[kiln.inventory.producers.scenario.compute]
- [x] [serial] [depends:kiln.inventory.collection] Add the Animus serving snapshot adapter with distinct states, request capacity provenance, withheld endpoints and hosted placement. r[kiln.inventory.producers.scenario.serving]
- [x] [serial] [depends:kiln.inventory.producers.scenario.serving] Refuse unsupported, malformed, conflicting and ambiguous producer documents in every adapter. r[kiln.inventory.producers.scenario.refused]

## Phase 3: Collection shell and fixtures

- [x] [serial] [depends:kiln.inventory.producers] Add the bounded explicit collection manifest shell, its observation port and the `inventory-collect` command. r[kiln.inventory.collection.scenario.refused]
- [x] [serial] [depends:kiln.inventory.collection.scenario.refused] Add the typed Nickel producer profile, captured producer fixtures and the criterion suite, including CLI, API and offline parity and host-name separation. r[kiln.inventory.collection.scenario.parity] r[kiln.inventory.collection.scenario.hostname]
- [x] [serial] [depends:kiln.inventory.collection.scenario.parity] Document the producer boundary and consumer contract, and run the repository gates. r[kiln.inventory.producers]
