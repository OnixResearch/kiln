# Inventory producer collection

Inventory v2 (`kiln.observe.inventory.v2`) separates agent definitions, worker
instances, compute and model serving. Kiln #60 asks that these inventories come
from producer observations. This page describes how producer documents become
v2 facts, which facts each producer leaves unknown, and what Kiln expects from
each producer. The change packages are
`.cairn/archive/2026-11-20-consume-inventory-producer-observations/` (intake)
and `.cairn/changes/consume-lattice-inventory-v2/` (Lattice v2 reader).

Everything here is read-only. Collection does not reserve, dispatch, load
models, route requests or contact a provider. All committed producer inputs are
deterministic fixtures. Live collection, serving and capacity qualification
remain with [Kiln #49](https://github.com/OnixResearch/kiln/issues/49).

## Producers and contracts

| Inventory | Producer contract | Adapter | Reviewed source |
|---|---|---|---|
| Agent definitions | `lattice.worker-inventory.v2` → `profile` (`lattice.worker-profile-facts.v2`) | `kiln_adapter_lattice::worker_inventory` | Lattice `9a74e7ef` (`docs/worker-inventory.md`) |
| Worker instances | `lattice.worker-inventory.v2` → `instance` (`lattice.worker-instance-observation.v2`) | `kiln_adapter_lattice::worker_inventory` | Lattice `9a74e7ef` |
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
kiln-web-service inventory /tmp/inventory-input.json operator 1795208985000 json
kiln-web-service inventory /tmp/inventory-input.json operator 1795208985000 html
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

Kiln reads `lattice.worker-inventory.v2` only. Before the strict decode it reads
the envelope, profile and instance schema labels. A v1 document, or any other
version, is refused with `lattice_inventory_unsupported_schema`.

A `kind: agents` source maps each definition identity to one definition.
Identical repeats collapse. A different fact for one definition at one revision
is refused with `lattice_profile_conflict`. One definition at two revisions in
one source is refused with `lattice_definition_revision_ambiguous`, because the
revision selected for new sessions is unpublished until Lattice #25. Other
revisions stay visible through the workers that pin them.

| v2 fact | From Lattice |
|---|---|
| definition id | `profile.definition.identity` (`blake3:` + 64 hex), stable across revisions |
| `profile_revision` | `profile.revision` |
| `harnesses` | `adapter` when declared or qualified |
| `output_contracts` | `outputs` when declared or qualified |
| `task_families`, `tools` | the sorted names when declared or qualified; a known empty list when `unavailable`; unknown when `unknown` |
| `permitted_models` | empty for `no_model`; unknown for broker routes, because a route identity is not a model |
| `revisions` | the five Lattice #25 slots, which must be `unknown` and owned by `lattice#25`; always unknown |
| `qualifications` | always unknown; `qualification_ref`, when bound, marks the revision qualified with that evidence |
| `authority` | `unknown` |
| `wait_reasons` | the profile's definition-scoped reasons: `capability_undeclared`, `qualification_unbound`, `revision_selection_unpublished` |

A profile's serving reasons describe one inspection of the serving owner, not
the definition. Serving facts come from the serving producer. The definition
name and basis are validated but not projected.

A `kind: workers` source maps each retained instance to one worker keyed by its
incarnation. A document without an instance yields no worker, because a
configured profile is never a running instance. The worker's `definition` is
the instance's `definition_identity` in the agents join source, and
`pinned_revision` is its `profile_revision`. `attempt` and `fence` are
Lattice's values, unchanged. Repeated observations of one incarnation are
ordered by `liveness.retained_generation`. The greater generation wins.
Equal generations with different facts are refused with
`lattice_instance_conflict`.

| Lattice lifecycle | Liveness `state` (`unknown_cause`) | v2 lifecycle |
|---|---|---|
| `admitted_not_dispatched` | `not_started` | `reserved` |
| `binding_rejected_before_dispatch` | `not_started` | `stopped` |
| `terminal_observed` | `exited` | `stopped` |
| `dispatch_recorded_outcome_unknown` | `unknown` (`unobserved` or `stale`) | `unknown`, with `liveness` listed as unknown |

Any other pairing is refused with `lattice_liveness_inconsistent`. The runtime
is `single_shot`. `idle` and `live` are not in Lattice's vocabulary and are
refused as malformed. No Lattice state maps to `idle`, `healthy` or
`executing`. Host reachability and allocation liveness are owned unknowns
(`kiln#49`), and they are always unknown. An unobserved cancellation request
is also unknown. The execution reference is the exact allocation id in the
compute join source. A host-local worker carries an unknown `host-local`
reference and never joins compute. Model services are the service ids of
served routes. Without a serving reading they are unknown, never empty. A
worker carries all of its instance's wait reasons. Workspace paths, grant
references and backend handles are not projected.

### Wait reasons

A wait reason keeps Lattice's exact code, its subject (a fact field or a route
named in the same document's `model_routes`) and its owner. It carries no free
text. Kiln checks each reason against the published vocabulary:

- the owner is fixed per code;
- the code is allowed in its section, and the subject fits the code;
- there are at most 32 reasons and no duplicates;
- terminal and rejected instances carry none;
- `capability_undeclared` appears exactly when its set is unknown;
- `liveness_unobserved` and `liveness_stale` match the liveness cause.

Anything else is refused with `lattice_wait_reason_inconsistent` or
`lattice_wait_reason_bound`. Inventory v2 carries a reason as a
provider-neutral `{code, subject, owner}` on its definition or worker. The
readiness explanation reports it as a blocker with the exact code, the fact
as subject, `fact:<field>` or `route:<name>` as constraint, and the `owner`.
Kiln's own blockers carry no owner. A reason is an explanation. It is never a
reservation or a dispatch decision.

Kiln also refuses these v2 inputs:

| Input | Refusal |
|---|---|
| Unknown fields, or out-of-vocabulary codes, fields, support values, bases, runtimes or liveness states | `lattice_inventory_malformed` |
| A definition identity that is not `blake3:` plus 64 lowercase hex characters | `lattice_definition_identity_invalid` |
| A definition name outside 1 to 64 bytes of `[a-z0-9][a-z0-9._:-]*` | `lattice_definition_name_invalid` |
| A declared or qualified set that is empty, has more than 32 names, is unsorted or repeated, or has a name outside the grammar | `lattice_capability_invalid` |
| A support value that disagrees with its value | `lattice_fact_support_inconsistent` |
| A published revision slot, or a slot owner other than `lattice#25` | `lattice_revision_unsupported` |
| An instance whose definition identity differs from its profile's | `lattice_instance_definition_mismatch` |
| An instance whose profile revision differs from its profile's | `lattice_instance_profile_mismatch` |

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

With Lattice v2, the definition identity, declared task families and tools,
unavailable capability (a known negative), single-shot liveness and wait
reasons are producer-derived. Exact work that a definition declares support
for is no longer blocked on capability. A family or tool missing from a
declared set is reported as `unsupported_task_family` or
`unsupported_agent_tool` with the exact name. An undeclared set is reported as
`agent_task_family_unknown` or `agent_tool_unknown`, beside Lattice's
`capability_undeclared` reason owned by `configuration`, and never as
unsupported. A worker pinned to another revision of the definition is reported
as `worker_pinned_other_revision`.

No producer-derived combination can yet be observed eligible. The definition
is still unknown in its revisions (`revision_selection_unpublished`,
`lattice#25`) and qualifications (`qualification_unbound`, `animus#18`), and its
authority is unpublished. Workers are single-shot and never idle. Their
liveness, host reachability and allocation liveness are unknown (`kiln#49`).
Pool capacity, routing permission and policy are not collected, and fixtures
are synthetic. This is the intended conservative result. The explanation
names every exact blocker. Where Lattice publishes a reason, the blocker
carries its exact code and owner.

## Consumer contract for producers

| Producer | Kiln reads today | Kiln needs before a producer fact can be ready |
|---|---|---|
| Lattice #24 | `lattice.worker-inventory.v2`: definition identity apart from the profile revision; profile revision, adapter, outputs, routes, compute requirement; declared, qualified, unavailable or unknown task families and tools; definition-scoped wait reasons; instance incarnation, attempt, fence, binding, allocation id, served-route service ids, lifecycle, single-shot liveness with cause and retained generation, instance wait reasons | an observed liveness or idle signal for a reusable runtime (Lattice owns none today, so workers are never idle); definition authority for admission; a definition-name field if inventory v2 adds one |
| Lattice #25 | the five revision slots, read and required to be unknown with owner `lattice#25` | desired, prepared, qualified, selected-for-new-sessions and pinned revisions with generation and evidence, mapped into `revisions`; until then one agents source may carry one revision per definition |
| Lattice #6 | instance `attempt_id`, `fence`, `work_id`; `dispatch_outcome_unknown` and `cancellation_unobserved` reasons | an observed attempt outcome and cancellation, so that these reasons can clear |
| Aspen/Molten #42 | allocation, readback and offer facts through Lattice's DTO | pool capacity, allocatable and reserved amounts per accounting authority, and free offers that are not yet allocated |
| Animus #17/#18 | snapshot readiness, capacity evidence, placement reference; Lattice's serving reasons on workers (`animus#17`) and `qualification_unbound` on definitions (`animus#18`) | routing eligibility for a policy, artifact cache state, and qualification evidence bound to a profile revision and task family |

## Non-claims

- Fixture captures and derived fixtures are not live observations. The committed
  manifests set `synthetic: true`. The Lattice files under
  `fixtures/inventory/producers/lattice/` are verbatim v2 captures.
  `provenance.json` records the Lattice commit (`9a74e7ef`, published on
  Lattice main), the capture command and each file's BLAKE3 hash, and an
  adapter test checks them.
- Declared task families and tools are operator statements, not qualification
  or a grant. Task matching beyond exact names belongs to Lattice #26 and
  Kiln #83.
- Collection does not verify producer authenticity. Producer identities are
  compared exactly.
- A displayed explanation is never a reservation or dispatch grant. Owning
  services revalidate at reservation and dispatch.
