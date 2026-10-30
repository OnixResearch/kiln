## Phase 1: Import

- [x] [serial] Vendor the canonical files and DID wrapper from the pinned revision, add the five shims, the always-validating `refname!`, and the required dependency features. r[kiln.radicle.canonical.provenance]
- [x] [serial] Remove the two fixture-dependent test modules, retain the other upstream tests, and fix what they reveal (`serde_json` `preserve_order`). r[kiln.radicle.canonical.provenance]

## Phase 2: Provenance and rail

- [x] [serial] Record per-file digests, stripped modules, shims and license; add the drift-refusing importer and verify it is idempotent. r[kiln.radicle.canonical.provenance] r[kiln.radicle.canonical.provenance.scenario.refresh]
- [x] [serial] Add the `heartwood-canonical` flake check and the license-map row; show the Kiln workspace graph does not gain `sqlite`. r[kiln.radicle.canonical.rail] r[kiln.radicle.canonical.rail.scenario.tests]
- [x] [serial] Run the crate tests, clippy, formatting and the flake check; record evidence. r[kiln.radicle.canonical.rail]
