## Phase 1: Contract model

- [x] [serial] Add the standalone `no_std + alloc` source contract crate and typed error model. r[kiln.radicle.source_contract.boundary] r[kiln.radicle.source_contract.request]
- [x] [parallel] Implement pure request and receipt admission with exact identity, feature, linkage, and bound checks. r[kiln.radicle.source_contract.sigrefs] r[kiln.radicle.source_contract.snapshot] r[kiln.radicle.source_contract.linkage]
- [x] [parallel] Add positive and negative Rust tests for every rejection class. r[kiln.radicle.source_contract.request] r[kiln.radicle.source_contract.sigrefs] r[kiln.radicle.source_contract.snapshot] r[kiln.radicle.source_contract.linkage]

## Phase 2: Interchange and provenance

- [x] [serial] Add the typed Nickel contract and positive and negative producer fixtures. r[kiln.radicle.source_contract.request] r[kiln.radicle.source_contract.sigrefs] r[kiln.radicle.source_contract.snapshot]
- [x] [parallel] Add deterministic producer fixture generation and independent fixture checks. r[kiln.radicle.source_contract.linkage]
- [x] [parallel] Document the pinned Radicle cohort, exact signed-reference facts, consumer duties, and non-claims. r[kiln.radicle.source_contract.nonclaims]

## Phase 3: Gates and publication

- [x] [serial] Add dependency and wasm gates that keep provider runtime crates outside the contract graph. r[kiln.radicle.source_contract.boundary]
- [x] [serial] Run formatting, focused tests, workspace tests, strict Clippy, wasm, Nickel, dependency, Cairn, Tracey, and focused Nix gates. r[kiln.radicle.source_contract.boundary] r[kiln.radicle.source_contract.linkage]
- [x] [serial] Record evidence, sync accepted specs, archive the change, and rerun validation. r[kiln.radicle.source_contract.nonclaims]
