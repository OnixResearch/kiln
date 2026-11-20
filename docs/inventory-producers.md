# Inventory producer collection

Inventory v2 (`kiln.observe.inventory.v2`) separates agent definitions, worker
instances, compute and model serving. Kiln #60 asks that these inventories come
from producer observations. This page describes how producer documents become
v2 facts, which facts each producer leaves unknown, and what Kiln expects from
each producer. The change package is
`.cairn/changes/consume-inventory-producer-observations/`.

Everything here is read-only. Collection does not reserve, dispatch, load
models, route requests or contact a provider. All committed producer inputs are
deterministic fixtures. Live collection, serving and capacity qualification
remain with [Kiln #49](https://github.com/OnixResearch/kiln/issues/49).

## Producers and contracts

| Inventory | Producer contract | Adapter | Reviewed source |
|---|---|---|---|
| Agent definitions | `lattice.worker-inventory.v1` → `profile` (`lattice.worker-profile-facts.v1`) | `kiln_adapter_lattice::worker_inventory` | Lattice main `1bdacb35` |
| Worker instances | `lattice.worker-inventory.v1` → `instance` (`lattice.worker-instance-observation.v1`) | `kiln_adapter_lattice::worker_inventory` | Lattice main `1bdacb35` |
| Compute | `lattice.compute-allocation-observation.v1` (Lattice's consumer DTO over the Aspen/Molten #42 `compute-capacity-allocation-v2`, `-readback-v1` and `-offer-v2` records) | `kiln_adapter_lattice::compute_inventory` | Lattice main `1bdacb35` |
| Models / serving | `animus.serving-availability-snapshot.v1` | `kiln_adapter_animus` | Animus main `4071c65e` |

The compute mapping lives in the Lattice adapter because the only published
consumer DTO for #42 records is Lattice-owned. Kiln has no direct Aspen offer
contract yet. Every DTO mirrors its producer field for field and refuses unknown
fields, unsupported schemas and structurally inconsistent records with a named
reason before any fact is produced.

## Collection manifest

A collecting shell writes one `kiln.inventory.collection-set.v1` manifest plus
the producer documents it retained:

| Field | Meaning |
|---|---|
| `principal`, `synthetic` | The admitted principal, and whether the inputs are rehearsal data |
| `joins.agents`, `joins.compute`, `joins.serving` | Sources that scope cross-inventory references |
| `collections[].source`, `producer`, `scope`, `kind` | One source as the v2 profile allowlists it |
| `collections[].contract` | The producer contract; each contract maps only into its listed kinds |
| `collections[].generation`, `observed_at_ms`, `valid_until_ms` | The Kiln-side collection window; the v2 profile watermark rejects older generations |
| `collections[].complete`, `next_page`, `unavailable_reason` | Page and availability facts, reported as `incomplete` or `unavailable` |
| `collections[].provider_logical_now` | The producer's logical clock as the shell read it. When absent, producer validity is unknown |
| `collections[].documents` | Producer documents, as plain paths relative to the manifest |

The shell reads only the named regular files, under named byte bounds. Absolute
paths, `..` components, symbolic links and unknown manifest fields are refused.
An unavailable source must list no documents.

```console
kiln-web-service inventory-collect fixtures/inventory/producers/pilot.collection.json \
  fixtures/inventory/producers/profile.json > /tmp/inventory-input.json
kiln-web-service inventory /tmp/inventory-input.json operator 1795200625000 json
kiln-web-service inventory /tmp/inventory-input.json operator 1795200625000 html
kiln-web-service serve --profile PROFILE --inventory /tmp/inventory-input.json
```

`inventory-collect` admits the composed snapshot against the profile and prints
the `{profile, snapshot}` input. The existing CLI, `GET /api/inventory` and
offline HTML consume that input and share one projection identity.
`ExplicitCollectionPort` implements `InventoryObservationPort` over a manifest
for host shells. The profile is authored in
`config/fixtures/inventory-producers-positive.ncl`, and its committed export is
`fixtures/inventory/producers/profile.json`.

## Mapping

### Lattice profiles and instances

A `kind: agents` source maps each distinct profile revision to one definition.
Identical repeats collapse. Two different facts for one revision are refused
with `lattice_profile_conflict`.

| v2 fact | From Lattice |
|---|---|
| definition id, `profile_revision` | `profile.revision` |
| `harnesses` | `adapter` when declared or qualified |
| `output_contracts` | `outputs` when declared or qualified |
| `permitted_models` | empty for `no_model`; unknown for broker routes, because a route identity is not a model |
| `task_families`, `tools`, `revisions`, `qualifications` | always unknown; Lattice publishes none of them |
| `authority` | `unknown` |

A `kind: workers` source maps each retained instance to one worker keyed by its
incarnation. A document without an instance yields no worker, because a
configured profile is never a running instance. Two different states for one
incarnation are refused with `lattice_instance_conflict`.

| Lattice lifecycle | v2 lifecycle |
|---|---|
| `admitted_not_dispatched` | `reserved` |
| `dispatch_recorded_outcome_unknown` | `unknown` |
| `binding_rejected_before_dispatch`, `terminal_observed` | `stopped` |

No Lattice state maps to `idle`, `healthy` or `executing`. Host reachability and
allocation liveness are always unknown. An unobserved cancellation request is
also unknown. The execution reference is the exact allocation id in the compute
join source. A host-local worker carries an unknown `host-local` reference and
never joins compute. Model services are the service ids of served routes.
Without a serving reading they are unknown, never empty. Workspace paths, grant
references and backend handles are not projected.

### Compute allocations

Each non-released allocation becomes one compute fact keyed by its allocation
id. It carries the host, the device incarnation, `execution-allocation` or
`serving-deployment-allocation`, backends from offer compatibility,
qualification references and `shared_serving`, the serving allocation it
references. The fact is `healthy` only when it is active, the readback is fresh
and healthy, any offer is healthy and the producer clock is below `valid_until`.
An expired, quarantined or unavailable allocation is `unavailable`. Anything
else is unknown. Sandbox support is not published and stays unknown.

Each claimed pool becomes one pool keyed by `pool_id` and accounted by its
producer authority. A pool shared by execution and serving is one pool with
every claim listed. A serving deployment's claims appear once, however many
execution allocations reference it, and an execution allocation is never
charged for them. Capacity, allocatable and reserved amounts stay unknown,
because the observation carries none of them. One authority under two pool ids
is refused (`compute_accounting_identity_ambiguous`). So is one pool id under
two authorities (`compute_pool_authority_conflict`). When a host shows fewer
observed allocations than its readback row count, that host's pool allocations
are unknown. For a repeated allocation the greater observation sequence wins in
any order. An equal sequence with different content is refused, as is a second
port.

### Serving snapshots

Each service becomes one model service:

| v2 fact | From Animus |
|---|---|
| `artifact`, `claimed_model` | `model.model_id`, a claim and never weight verification |
| `configuration` | `model.configuration_ref` |
| `deployment` | `deployment_id@deployment_generation` |
| `endpoint_alias` | the service id; the endpoint identity is withheld |
| `backend` | the locality class (`local`, `private-remote`, `hosted-remote`) |
| `state` | `ready` → `warm`, or `saturated` when current request slots are zero; `loading` → `loading`; `absent` → `configured`; `draining`, `unavailable` → `unavailable`; `unknown`, `stale` → `unknown` |
| `available_requests` | exactly one current serving-owner `request-slots` or provider `request-quota` reading, only under an owner admission offer |
| `compute_devices` | the exact `resource_consumer_ref` allocation in the compute join source; empty for a hosted API |
| `accounting_pools` | filled by composition from that allocation's pools |

Queue depth, KV-cache tokens, token quotas and host probes never count as
available requests. Routing permission, artifact cache state and any unknown
capability are listed as unknown. A snapshot outside its validity window at the
recorded producer clock, or one without a clock, is marked unknown. The newest
`(source_generation, sequence)` per producer source wins.

## What the projection shows today

With the current producer contracts, no producer-derived combination can be
observed eligible. Pool capacity, routing permission, agent tools, task families
and worker liveness are unpublished, and no policy source is collected. This is
the intended conservative result. The explanation still names every exact
blocker: loading, saturation, unsupported context, sandbox and backend, unknown
execution and stale or missing providers.

## Consumer contract for producers

| Producer | Kiln reads today | Kiln needs before a producer fact can be ready |
|---|---|---|
| Lattice #24 | profile revision, adapter, outputs, routes, compute requirement; instance incarnation, attempt, fence, binding, allocation id, served-route service ids, lifecycle | a definition identity separate from the profile revision; declared task families and tools; an observed worker liveness or idle signal for reusable runtimes |
| Lattice #25 | — | desired, prepared, qualified, selected-for-new-sessions and pinned revisions with generation and evidence, mapped into `revisions` |
| Lattice #6 | instance `attempt_id`, `fence`, `work_id` | the same attempt and fence in the instance observation, unchanged |
| Aspen/Molten #42 | allocation, readback and offer facts through Lattice's DTO | pool capacity, allocatable and reserved amounts per accounting authority, and free offers that are not yet allocated |
| Animus #17/#18 | snapshot readiness, capacity evidence, placement reference | routing eligibility for a policy, and artifact cache and qualification facts |

## Non-claims

- Fixture captures and derived fixtures are not live observations. The committed
  manifests set `synthetic: true`.
- Collection does not verify producer authenticity. Producer identities are
  compared exactly.
- A displayed explanation is never a reservation or dispatch grant. Owning
  services revalidate at reservation and dispatch.
