## Phase 1: Core vocabulary

- [x] [serial] Add provider-neutral `WaitReason` facts on definitions and workers, owned explanation blockers and unknown-versus-unsupported capability blockers to `kiln_core::inventory`, omitted when empty, with admission bounds, owner grammar and duplicate refusal; test positive and negative admission, explanation and unchanged snapshot identity. r[kiln.inventory.producers.scenario.wait]

## Phase 2: Lattice v2 reader

- [x] [serial] [depends:kiln.inventory.producers.scenario.wait] Replace the v1 DTOs with `lattice.worker-inventory.v2`: refuse v1 as unsupported, key definitions by definition identity, join workers by `definition_identity`, keep attempt, fence and work unchanged, and map liveness into lifecycle ordered by retained generation without idle or live. r[kiln.inventory.producers.scenario.lattice]
- [x] [serial] [depends:kiln.inventory.producers.scenario.lattice] Map declared, qualified, unavailable and unknown task families and tools, the #25 revision slots and definition-scoped and instance wait reasons with their owners. r[kiln.inventory.producers.scenario.capability]
- [x] [serial] [depends:kiln.inventory.producers.scenario.capability] Refuse malformed v2 documents: unknown fields, out-of-vocabulary codes, owners, subjects, support values and liveness states, over-bound names, inconsistent liveness, definition mismatch, revision ambiguity and generation conflicts, each with positive counterparts. r[kiln.inventory.producers.scenario.refused]

## Phase 3: Fixtures, criteria and contract

- [x] [serial] [depends:kiln.inventory.producers.scenario.refused] Replace every v1 Lattice fixture with the verbatim v2 captures, record their provenance and BLAKE3 hashes, move the collection windows and re-derive the criterion suite and console rendering. r[kiln.inventory.producers]
- [x] [serial] [depends:kiln.inventory.producers] Update `docs/inventory-producers.md` and `CHANGELOG.md` with the v2 mapping and consumer contract, then run the repository gates and record the evidence. r[kiln.inventory.producers]
