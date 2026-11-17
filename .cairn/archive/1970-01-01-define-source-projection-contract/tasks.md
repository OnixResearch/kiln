## Phase 1: Contract

- [x] [serial] Add repository enrollment, locator, reconciliation and identity admission to `kiln_core::projection`, with cache-move, equal-name, conflict, reconciliation and alias cases. r[kiln.source.repository_identity] r[kiln.source.repository_identity.scenario.cache_move] r[kiln.source.repository_identity.scenario.conflict]
- [x] [serial] Add projection definition admission reusing monorepo scope admission, canonical identity and profile rules, with ordering and refusal cases. r[kiln.source.projection.definition] r[kiln.source.projection.definition.scenario.canonical] r[kiln.source.projection.definition.scenario.profile_rules]
- [x] [serial] Add the pure resolver, manifest, origin and content identities and sorted diagnostics, with reproducibility, alias, identical-bytes, identity and diagnostic cases. r[kiln.source.projection.manifest] r[kiln.source.projection.manifest.scenario.reproducible] r[kiln.source.projection.manifest.scenario.identity] r[kiln.source.projection.manifest.scenario.diagnostics]
- [x] [serial] Add the partial-view and completeness-witness type states and manifest verification, with refusal, forgery, staleness and `compile_fail` cases. r[kiln.source.projection.completeness] r[kiln.source.projection.completeness.scenario.partial] r[kiln.source.projection.completeness.scenario.forged]
- [x] [serial] Add the monorepo scope compatibility conversion and its agreement case. r[kiln.source.projection.compatibility] r[kiln.source.projection.compatibility.scenario.scope]

## Phase 2: Configuration and verification

- [x] [serial] Add the typed Nickel contract, positive and negative fixtures, the committed export, its freshness check and a Rust admission case over the export. r[kiln.source.projection.definition]
- [x] [serial] Run kiln-core tests and doctests, Clippy, rustfmt and the Nickel gate; update the operator guide and changelog, register the active delta root, and pass Cairn gates, validation and active-delta traceability. r[kiln.source.projection.manifest]
