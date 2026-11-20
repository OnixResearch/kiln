## Context

Inventory v2 (`kiln.observe.inventory.v2`, accepted in the `resource-inventory`
capability) admits a Kiln-vocabulary snapshot against a principal-specific
source allowlist. It evaluates freshness at a shell-supplied clock, explains
readiness for exact work, and projects one BLAKE3-identified view. The live API,
the CLI and offline HTML share that view. This change adds the producer side of
the boundary. The producers are:

| Producer | Contract | Reviewed source |
|---|---|---|
| Lattice profile and instance facts (#24) | `lattice.worker-inventory.v1` with `lattice.worker-profile-facts.v1` and `lattice.worker-instance-observation.v1` | Lattice main `1bdacb35`, `implementation-attempt inspect` |
| Compute allocations (Aspen/Molten #42) | `lattice.compute-allocation-observation.v1`, Lattice's consumer DTO over `compute-capacity-allocation-v2`, `-readback-v1` and `-offer-v2` | Lattice main `1bdacb35`, `docs/worker-compute-binding.md` |
| Serving availability (Animus #17) | `animus.serving-availability-snapshot.v1` | Animus main `4071c65e`, `docs/serving-admission.md` |

## Goals and Success Contract

- Producer documents map into the four v2 inventories without loss of exact
  identities. Every v2 fact the producer does not publish is listed in the
  fact's `unknown_fields`, and v2 already treats such a fact as unknown.
- A configured profile is a definition, never a worker. Lattice lifecycles map
  conservatively and never to `idle`, `healthy` or `executing`.
- A serving deployment's allocation is counted once, however many workers
  reference it. A pool shared by execution and serving is one pool.
- Joins use exact producer-scoped identities: allocation id, serving service id
  and resource consumer reference. Host names never join inventories.
- One collected snapshot gives the same projection identity through the CLI,
  the live API and offline HTML. The collection shell reads only named files.

## Decisions

### Decision: Extend inventory v2 rather than add a new profile

**Choice:** Producer adapters emit the existing v2 fact types, and v2 admission,
explanation and projection are reused unchanged.

**Rationale:** v2 already encodes the issue's states, blockers, accounting and
consumer parity. A second vocabulary would split operator views. Composition is
a new pure function beside v2, so existing snapshots keep their identity.

### Decision: Kiln-owned collection envelope

**Choice:** `kiln.inventory.collection-set.v1` names the principal, the
synthetic flag, the cross-inventory join sources and, for each source, its
producer, scope, kind, producer contract, collection generation, collection
window, completeness, next page, unavailable reason and optional provider
logical clock. Each source lists producer documents by relative path.

**Rationale:** The producers do not share a clock or generation domain. Lattice
reports wall-clock observation time, while Animus and Aspen use logical ticks.
The collecting shell owns the Kiln-side collection window and generation, and
the v2 source allowlist enforces its watermark. Producer logical validity is
evaluated against the provider clock the shell recorded. When that clock is
absent, producer validity is unknown. Relative paths keep producer bytes
verbatim and the offline shell bounded. Absolute paths, `..`, non-regular
files, oversized files and unknown fields are refused.

### Decision: Lattice mapping

**Choice:** A `kind: agents` source maps each distinct profile revision to one
definition. Identical repeats deduplicate and conflicting facts refuse. Task
families, tools, revision lifecycle and qualification applicability are always
unknown. Output contracts, harness and permitted models are unknown unless the
fact is declared or qualified. A `kind: workers` source maps each retained
instance to a worker keyed by its incarnation. The worker carries its definition
revision, attempt, fence and exact execution reference: the allocation id in the
join compute source, or an unknown host-local reference. Its model services are
the served route service ids. Lifecycle maps
`admitted_not_dispatched` to `reserved`,
`binding_rejected_before_dispatch` and `terminal_observed` to `stopped`, and
`dispatch_recorded_outcome_unknown` to `unknown`. Host reachability and
allocation liveness stay unknown because Lattice reports them unknown.

**Rationale:** Lattice states that inspection never reports idle, running,
healthy or available. A document without an instance is a definition only.

### Decision: Compute mapping

**Choice:** Each non-released allocation becomes a compute fact keyed by its
allocation id, with host, device incarnation, kind, health, backends and
qualification references. Health is `healthy` only when status is active, the
readback is fresh and healthy, any offer is healthy and the recorded provider
clock is below `valid_until`. Each claimed pool becomes one pool fact keyed by
`pool_id` and accounted by its authority id. A second pool id with the same
authority, or a pool id with two authorities, is refused. Pool capacity,
allocatable and reserved amounts stay unknown because the observation does not
carry them. A host with fewer observed allocations than its readback row count
marks those pools' allocations incomplete. A repeated allocation keeps the
greater observation sequence. An equal sequence with different content is
refused. An execution allocation's shared serving reference is never charged.
It is shown as a new optional `shared_serving` list on the compute fact. The
list is omitted when empty, so existing v2 snapshots keep their encoding. The
mapping lives in `kiln-adapter-lattice`, because the accepted DTO is
Lattice-owned. That also keeps it out of the Octet-linted Aspen runtime
workspace, which carries no inventory fixtures.

**Rationale:** Execution and serving share physical pools under one producer
accounting authority. Kiln never derives free capacity from reservation facts
alone. A worker's use of a shared deployment is visible through its exact
allocation without being inferred as a route-level model binding.

### Decision: Serving mapping

**Choice:** Each Animus service becomes one model service. The artifact and
claimed model are the model id. The configuration is the configuration
reference. The deployment is `deployment_id@generation`. The endpoint alias is
the service id, so endpoint identity is withheld. Replicas and context come from
the snapshot. The backend is the locality class. State maps `ready` to `warm`,
or to `saturated` when current serving-owner request slots are zero. `loading`
maps to `loading`, `absent` to `configured`, `draining` and `unavailable` to
`unavailable`, and `unknown` and `stale` to `unknown`. Available requests come
only from current serving-owner request slots or a provider request quota. Queue
depth, KV cache and host probes never count. Supported capabilities become
tokens, and unknown capabilities are listed as unknown fields. Routing
permission is unknown because Animus owns routing eligibility. Artifact cache
state and weight verification are absent. A `resource_consumer_ref` becomes
the exact compute allocation reference. A hosted service without one has no
compute devices.

**Rationale:** The snapshot reports serving readiness and capacity evidence, not
reservations, routing grants or weight identity.

### Decision: Composition

**Choice:** `kiln_core::inventory::compose_snapshot` takes per-source fragments.
It rejects duplicate sources, kind mismatches and duplicate facts. It sets a
model's accounting pools to the union of its exact compute allocations' pools.
It sorts every list deterministically. The web-service shell implements
`InventoryObservationPort` over a manifest. `inventory-collect MANIFEST PROFILE`
prints the existing inventory input.

**Rationale:** Adapters stay provider-specific. Joining stays pure and
provider-neutral in the core.

## Verification Strategy

Use pure adapter tests with positive and negative cases for each producer:
unsupported schema, unknown fields, malformed values, conflicting profiles,
accounting ambiguity, out-of-order sequence and expired or missing clocks. A
web-service suite collects the committed producer fixtures and maps every #60
criterion to an assertion over the resulting projection. CLI, API and offline
HTML must produce one projection identity. Nickel fixtures check the producer
profile export. Repository gates run once at the end.

## Risks and Trade-offs

- Producer fixtures are deterministic captures, not live observations.
  `synthetic` stays true in the committed manifest, and live qualification
  remains with Kiln #49.
- With the current producer contracts, no producer-derived combination can be
  observed eligible: pool capacity, routing permission, tools and liveness are
  unpublished. This is the intended conservative result, and the consumer
  contract names the missing producer facts.
