## Phase 1: Core vocabulary

- [x] [serial] Add provider-neutral `RevisionManagement`, `TaskFamilyQualification` with `EvidenceProvenance`, `RevisionPin` with `PinStatus`, the explanation's task-family qualification and the `task_family_qualification_not_live` and `worker_pin_blocked` blockers to `kiln_core::inventory`, omitted when empty, with admission of tokens, bounds and order; test positive and negative admission, explanation and unchanged snapshot identity. r[kiln.inventory.producers.scenario.qualification]

## Phase 2: Lattice v3 reader

- [x] [serial] [depends:kiln.inventory.producers.scenario.qualification] Replace the v2 DTOs with `lattice.worker-inventory.v3`: refuse v2 as unsupported, decode the unmanaged, unknown and recorded revision slots, map them with capability qualifications and pins onto definitions and workers, and keep the definition keyed by identity with workers joined unchanged. r[kiln.inventory.producers.scenario.lattice]
- [x] [serial] [depends:kiln.inventory.producers.scenario.lattice] Replace the two-revision refusal with the selected-versus-pinned model, refusing two selected revisions and other contradictory lifecycle facts, and derive owner-attributed reasons for unknown slots. r[kiln.inventory.producers.scenario.revisions]
- [x] [serial] [depends:kiln.inventory.producers.scenario.revisions] Validate the v3 wait-reason vocabulary (`revision_not_admitted`, `qualification_unbound` on `capability_qualifications`) and refuse malformed v3 documents, each with positive counterparts. r[kiln.inventory.producers.scenario.refused]

## Phase 3: Fixtures, criteria and contract

- [x] [serial] [depends:kiln.inventory.producers.scenario.refused] Replace every v2 Lattice fixture with the verbatim v3 captures, record their provenance and BLAKE3 hashes, move the collection windows, and re-derive the criterion suite and console rendering with a managed definition. r[kiln.inventory.producers]
- [x] [serial] [depends:kiln.inventory.producers] Update `docs/inventory-producers.md` and `CHANGELOG.md` with the v3 mapping and consumer contract, then run the repository gates and record the evidence. r[kiln.inventory.producers]
