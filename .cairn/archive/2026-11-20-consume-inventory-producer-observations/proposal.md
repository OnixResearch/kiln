## Why

Kiln #60 asks for separate agent, worker, compute and model-serving
inventories with task readiness explanations. The read-only inventory v2 core
(`kiln_core::inventory`), its explicit-file shell, the authenticated API and the
offline HTML consumer already exist. Their inputs are hand-authored Kiln
snapshots. No producer observation reaches them: nothing maps Lattice
`implementation-attempt inspect` output, the compute allocation observations of
Aspen/Molten #42, or Animus #17 serving availability snapshots into the
admitted v2 facts.

Without that intake an operator cannot see what the producers actually report.
Producers also publish fewer facts than the v2 vocabulary names. Lattice reports
no task families, tools or liveness. The compute observation carries no pool
capacity. Animus carries no routing permission or artifact cache state. Kiln
must show each missing fact as explicitly unknown rather than inventing it.

## What Changes

- Add a pure provider-neutral composition step to `kiln_core::inventory`. It
  joins per-source fragments into one v2 snapshot by exact producer-scoped
  identities. It fills a serving fact's accounting pools only from its exact
  compute allocation. It rejects ambiguous sources and facts.
- Add producer adapters that map one source's documents into v2 facts:
  - `kiln-adapter-lattice::worker_inventory` maps `lattice.worker-inventory.v1`.
    Profiles become definitions and retained instances become workers.
  - `kiln-adapter-lattice::compute_inventory` maps
    `lattice.compute-allocation-observation.v1` into allocation facts and pools
    keyed by accounting authority. That document is Lattice's published
    consumer DTO over the #42 producer records; Kiln has no direct Aspen offer
    contract yet, so the mapping lives beside the other Lattice DTOs.
  - A new `kiln-adapter-animus` maps `animus.serving-availability-snapshot.v1`
    services into model-service facts.
  Every fact the producer does not publish is listed in `unknown_fields`.
- Add a bounded explicit-file collection shell in `kiln-web-service`. It reads
  `kiln.inventory.collection-set.v1` manifests that name producer documents
  relative to the manifest, and implements the existing
  `InventoryObservationPort`. A new `inventory-collect` command emits the
  `{profile, snapshot}` input. The existing CLI, API and offline HTML consume
  that input.
- Add a typed Nickel producer profile with a committed export, plus producer
  fixtures captured from real Lattice `inspect` output at 1bdacb35, copied from
  Lattice's compute observation fixtures and copied from Animus's snapshot
  fixture at 4071c65e. Add a criterion-by-criterion suite with negative cases.

## Impact

Affected: `kiln-core` inventory module, `kiln-adapter-lattice`, new
`kiln-adapter-animus`, `kiln-web-service` inventory
shell and CLI, `config/`, `fixtures/inventory/producers/`,
`scripts/check-nickel-fixtures.sh` and docs. Inventory v2 semantics, canonical
observation v1, worker slot v1, fair admission, work claims, acceptance gates and
dispatch are unchanged. No provider is contacted. Live provider collection,
serving and capacity qualification remain with Kiln #49.
