## Why

Kiln #60 reads Lattice's worker inventory through
`kiln_adapter_lattice::worker_inventory`, which accepts only
`lattice.worker-inventory.v2`. v2 publishes the five Lattice #25 revision
slots as owned unknowns, so every Lattice definition is blocked by
`revision_selection_unpublished` (`lattice#25`), and Kiln refuses two
revisions of one definition in one source because no selection is published.

Lattice #25 now publishes `lattice.worker-inventory.v3`
(`docs/worker-inventory.md` and `docs/agent-revisions.md` in Lattice). With a
revision store, v3 records the definition's desired revision, prepared
revisions, qualifications carrying their Animus #18 attachment reference,
evidence class and `fixture` or `live` provenance, the revision selected for
new sessions, and each instance's pinned revision. Without a lifecycle every
slot is `unmanaged`, which is not `unknown`. v3 also stabilizes the revision:
v2 hashed the task prompt and base snapshot, so each attempt had its own
revision. Lattice emits no v2 output, and Kiln's v2 DTOs refuse v3. The owner
approved the cutover on 2026-11-20.

## What Changes

- `kiln-adapter-lattice::worker_inventory` reads `lattice.worker-inventory.v3`
  only. A v2 document is refused with `lattice_inventory_unsupported_schema`.
- The four profile revision slots and the instance's `pinned` slot map into
  the definition's revision state and a worker revision pin. `unmanaged`,
  `unknown` (with its owner) and `recorded` stay distinct.
- A definition with a recorded selection may appear at several revisions in
  one source: Kiln projects the selected revision, and workers pinned to the
  others stay visible. Contradictory lifecycle facts, such as two selected
  revisions for one definition, are refused.
- Task-family qualifications carry their reference, evidence class and
  provenance. The readiness explanation names the needed family's current
  reference with its provenance, and a `fixture` reference never counts as
  live evidence.
- The `revision_selection_unpublished` wait reason is gone. Facts v3 still
  leaves unknown keep an owner-attributed reason (preparation `kiln#49`,
  qualification `animus#18`, selection `lattice#25`).
- `kiln_core::inventory` gains provider-neutral optional fields for the
  revision management state, task-family qualifications, the worker revision
  pin and the explanation's task-family qualification. All are omitted when
  empty, so existing snapshots keep their identity.
- The v2 fixtures are replaced by the 16 v3 captures, copied verbatim, with a
  provenance record checked by BLAKE3.

## Impact

- **Files**: `crates/kiln-core/src/inventory.rs` and its tests,
  `crates/kiln-adapter-lattice/src/worker_inventory.rs`,
  `worker_inventory_wait.rs` and `worker_inventory_tests.rs`,
  `crates/kiln-web-service/src/inventory_collection_tests.rs`,
  `crates/kiln-web-service/src/inventory.js`, `tests/inventory-ui.test.mjs`,
  `fixtures/inventory/producers/`, `docs/inventory-producers.md`,
  `.cairn/specs/resource-inventory/spec.md`, `CHANGELOG.md` and this change
  package.
- **Testing**: `cargo fmt -- --check`; `cargo clippy --locked --workspace
  --all-targets -- -D warnings`; `cargo test --locked --workspace
  --no-fail-fast`; the node inventory UI tests;
  `scripts/check-nickel-fixtures.sh`; `cairn validate`, the change gates and
  `cairn traceability coverage`; the Octet flake check.
- **Unchanged**: compute and serving adapters, the collection manifest
  format, worker slot v1, dispatch and reservations. No provider is
  contacted. Live qualification stays with Animus #18 and Kiln #49.
