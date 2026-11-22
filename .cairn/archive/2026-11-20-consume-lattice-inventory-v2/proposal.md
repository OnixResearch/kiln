## Why

Kiln #60 reads Lattice's worker inventory through
`kiln_adapter_lattice::worker_inventory`. That reader accepts only
`lattice.worker-inventory.v1`. v1 publishes no definition identity apart from
the profile revision, no task families or tools, no worker liveness and no
wait reasons, so Kiln lists all of them as unknown and every explanation is
blocked by Kiln's own unknowns.

Lattice now publishes `lattice.worker-inventory.v2` (Lattice commit
`9a74e7ef1da4e0ca7610dce0db039b3825a7154d`, `docs/worker-inventory.md`). v2
adds a definition identity that is stable across revisions, declared,
qualified, unavailable or unknown task families and tools, owned unknown
revision slots for Lattice #25, single-shot liveness with an unknown cause and
a retained generation, and closed-vocabulary wait reasons with fixed owners.
Lattice emits no v1 output, and Kiln's v1 reader refuses v2 documents. The
owner approved the cutover on 2026-11-20.

## What Changes

- `kiln-adapter-lattice::worker_inventory` reads `lattice.worker-inventory.v2`
  only. A v1 document is refused with `lattice_inventory_unsupported_schema`.
- A definition is keyed by `definition.identity`, separate from its profile
  revision. Workers join it through the instance's `definition_identity`.
  Attempt, fence and work id are unchanged.
- Declared or qualified task families and tools become known capability.
  `unavailable` becomes a known negative. `unknown` stays unknown.
- The five #25 revision slots map to the existing revision fields as unknown.
- Single-shot liveness maps to the worker lifecycle and never to idle or live.
  An unknown liveness is listed as unknown, and the retained generation orders
  repeated observations of one incarnation.
- Producer wait reasons keep their exact code, subject and owner. Definitions
  and workers carry them, and the readiness explanation reports them as
  blockers with their owner. To carry them, `kiln_core::inventory` gains a
  provider-neutral `WaitReason` and an optional `owner` on `Blocker`. Both are
  omitted when empty, so existing v2 snapshots keep their identity.
- Malformed v2 documents are refused: unknown fields, codes, subjects, owners,
  support values, names or liveness states outside the published vocabulary,
  names over the bounds, a definition identity that differs between profile
  and instance, and one definition at two revisions in one source.
- Every v1 Lattice fixture is replaced by the v2 captures, copied verbatim. A
  provenance record names the Lattice commit and each capture's BLAKE3 hash.
  The criterion suite and `docs/inventory-producers.md` are re-derived.

## Impact

- **Files**: `crates/kiln-core/src/inventory.rs` and its tests,
  `crates/kiln-adapter-lattice/src/worker_inventory.rs` and its tests,
  `crates/kiln-web-service/src/inventory_collection_tests.rs`,
  `crates/kiln-web-service/src/inventory.js`, `tests/inventory-ui.test.mjs`,
  `fixtures/inventory/producers/`, `docs/inventory-producers.md`,
  `CHANGELOG.md` and this change package.
- **Testing**: `cargo fmt -- --check`; `cargo clippy --locked --workspace
  --all-targets -- -D warnings`; `cargo test --locked --workspace
  --no-fail-fast`; `scripts/check-nickel-fixtures.sh`; the node inventory UI
  tests; `cairn validate` and the change gates; `cairn traceability coverage`;
  the Octet flake check.
- **Unchanged**: compute and serving adapters, inventory v2 admission
  semantics for existing facts, the collection manifest, worker slot v1,
  dispatch and reservations. No provider is contacted. Live qualification
  stays with Kiln #49.
