# Inventory producer collection

Inventory v2 (`kiln.observe.inventory.v2`) separates agent definitions, worker
instances, compute and model serving. Kiln #60 asks that these inventories come
from producer observations. This page describes how producer documents become
v2 facts, which facts each producer leaves unknown, and what Kiln expects from
each producer. The change packages are
`.cairn/archive/2026-11-20-consume-inventory-producer-observations/` (intake),
`.cairn/archive/2026-11-20-consume-lattice-inventory-v2/` (Lattice v2 reader)
and `.cairn/archive/2026-11-21-consume-lattice-inventory-v3/` (Lattice v3 reader).

Everything here is read-only. Collection does not reserve, dispatch, load
models, route requests or contact a provider. All committed producer inputs are
deterministic fixtures. Live collection, serving and capacity qualification
remain with [Kiln #49](https://github.com/OnixResearch/kiln/issues/49).

## Producers and contracts

| Inventory | Producer contract | Adapter | Reviewed source |
|---|---|---|---|
| Agent definitions | `lattice.worker-inventory.v3` → `profile` (`lattice.worker-profile-facts.v3`) | `kiln_adapter_lattice::worker_inventory` | Lattice `ed51aee2` (`docs/worker-inventory.md`, `docs/agent-revisions.md`) |
| Worker instances | `lattice.worker-inventory.v3` → `instance` (`lattice.worker-instance-observation.v3`) | `kiln_adapter_lattice::worker_inventory` | as above |
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
kiln-web-service inventory /tmp/inventory-input.json operator 1795237895000 json
kiln-web-service inventory /tmp/inventory-input.json operator 1795237895000 html
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

Kiln reads `lattice.worker-inventory.v3` only. Before the strict decode it reads
the envelope, profile and instance schema labels. A v1 or v2 document, or any
other version, is refused with `lattice_inventory_unsupported_schema`. v2
revisions were per-attempt, because they also framed the task prompt, base
snapshot and broker expiry; v3 frames only profile-level configuration and the
reviewed harness template, so one revision serves many attempts. A v2
revision is never compared with a v3 one.

A `kind: agents` source maps each definition identity to one definition.
Identical repeats collapse. A different fact for one definition at one revision
is refused with `lattice_profile_conflict`. Every document of one definition in
one source, in agents and workers sources alike, must carry the same revision
lifecycle. Two different recorded selections are two selected revisions and
are refused with `lattice_definition_selection_conflict`; any other difference
is refused with `lattice_definition_lifecycle_conflict`. Several revisions of
one definition may then coexist: Kiln projects the definition at its only
revision, or at the recorded `selected_for_new_sessions` revision. Several
revisions without a recorded selection, or with a selection none of them is,
are refused with `lattice_definition_revision_ambiguous`. Older revisions stay
visible through the workers that pin them.

| v2 fact | From Lattice |
|---|---|
| definition id | `profile.definition.identity` (`blake3:` + 64 hex), stable across revisions |
| `profile_revision` | `profile.revision` |
| `harnesses` | `adapter` when declared |
| `output_contracts` | `outputs` when declared |
| `task_families`, `tools` | the sorted names when declared (or qualified, for task families); a known empty list when `unavailable`; unknown when `unknown` |
| `permitted_models` | empty for `no_model`; unknown for broker routes, because a route identity is not a model |
| `revisions.management` | `unmanaged` when every slot is `unmanaged`, else `managed` |
| `revisions.desired` | recorded `desired.revision` |
| `revisions.prepared` | the profile's revision when the recorded `prepared` list holds it |
| `revisions.qualified` | the profile's revision when `capability_qualifications` is not empty |
| `revisions.selected_for_new_sessions` | recorded `selected_for_new_sessions.revision` |
| `revisions.generation` | the selection's `rollout_generation`; 0 without a selection |
| `revisions.evidence` | the `qualification_ref` of each current capability qualification |
| `revisions.preparation_failure` | none; Lattice publishes no failed preparation |
| `task_family_qualifications` | `capability_qualifications`: task family, `qualification_ref` (the Animus #18 attachment identity), `evidence_class` and `provenance` (`fixture` or `live`) |
| `qualifications` | always unknown: a reference is not a subject, corpus, protocol and contract record |
| `authority` | `unknown` |
| `wait_reasons` | the profile's definition-scoped reasons (`capability_undeclared`, `qualification_unbound`, `revision_not_admitted`), plus `revision_fact_unknown` on `revisions.<slot>` with the slot's owner for each `unknown` slot |

A revision slot is `unmanaged`, `unknown` with an owner, or `recorded`, and the
three stay distinct. An `unmanaged` slot is empty and known: it is not listed
as unknown and carries no reason, because no lifecycle governs the definition.
An `unknown` slot is listed in `unknown_fields` as `revisions.<slot>` and keeps
its owner: `prepared` `kiln#49`, `qualified` `animus#18`, `desired` and
`selected_for_new_sessions` `lattice#25`. A revoked revision is not a slot; it
shows as empty `capability_qualifications`, `revision_not_admitted` and blocked
pins. A profile's serving reasons describe one inspection of the serving owner,
not the definition. Serving facts come from the serving producer. The
definition name and basis are validated but not projected.

A `kind: workers` source maps each retained instance to one worker keyed by its
incarnation. A document without an instance yields no worker, because a
configured profile is never a running instance. The worker's `definition` is
the instance's `definition_identity` in the agents join source, and
`pinned_revision` is its `profile_revision`, which a recorded pin must equal.
`pin` is `{state: unmanaged}` for an attempt admitted outside a lifecycle, or
`{state: recorded, qualification, rollout_generation, identity, status,
blocked}` from the lifecycle's pin, with status `live`, `closed` or
`released`. An `unknown` pin leaves `pin` absent, lists `pinned` as unknown and
adds `revision_fact_unknown` on `pinned` (`lattice#25`). `attempt` and `fence`
are Lattice's values, unchanged. Repeated observations of one incarnation are
ordered by `liveness.retained_generation`. The greater generation wins.
Equal generations with different facts are refused with
`lattice_instance_conflict`, including one observation before and one after a
revocation.

| Lattice lifecycle | Liveness `state` (`unknown_cause`) | v2 lifecycle |
|---|---|---|
| `admitted_not_dispatched` | `not_started` | `reserved` |
| `binding_rejected_before_dispatch` | `not_started` | `stopped` |
| `revision_blocked_before_dispatch` | `not_started` | `stopped` |
| `terminal_observed` | `exited` | `stopped` |
| `dispatch_recorded_outcome_unknown` | `unknown` (`unobserved` or `stale`) | `unknown`, with `liveness` listed as unknown |

Any other pairing is refused with `lattice_liveness_inconsistent`. The runtime
is `single_shot`. `idle` and `live` are not in Lattice's liveness vocabulary
and are refused as malformed (a pin's `live` status describes the session's
pin, not the process). No Lattice state maps to `idle`, `healthy` or
`executing`. Host reachability and allocation liveness are owned unknowns
(`kiln#49`), and they are always unknown. An unobserved cancellation request
is also unknown. The execution reference is the exact allocation id in the
compute join source. A host-local worker carries an unknown `host-local`
reference and never joins compute. Model services are the service ids of
served routes. Without a serving reading they are unknown, never empty. A
worker carries all of its instance's wait reasons. Workspace paths, grant
references and backend handles are not projected.

### Revisions and qualifications in readiness

The readiness explanation reports `revision_not_selected_for_new_sessions`
unless the definition's projected revision is the selected one, and
`worker_pinned_other_revision` for a worker pinned to another revision. A
recorded blocked pin adds `worker_pin_blocked` with the pin identity to each of
that worker's candidates. The explanation's `task_family_qualification` is the
needed task family's reference, with its evidence class and provenance, only
when the definition is at the needed revision and its fact is not missing,
stale or in the future. A `fixture` reference adds
`task_family_qualification_not_live` with the reference as constraint, so
fixture evidence never counts as live. A reference is not a full
qualification record, so `qualification_missing_stale_or_mismatched` still
applies.

### Wait reasons

A wait reason keeps Lattice's exact code, its subject (a fact field or a route
named in the same document's `model_routes`) and its owner. It carries no free
text. Kiln checks each reason against the published vocabulary:

- the owner is fixed per code;
- the code is allowed in its section, and the subject fits the code;
- there are at most 32 reasons and no duplicates;
- terminal, rejected and revision-blocked instances carry none;
- `capability_undeclared` appears exactly when its set is unknown;
- `qualification_unbound` appears exactly when `task_families` is not
  `qualified`;
- `revision_not_admitted` never appears on an unmanaged profile, appears on a
  managed one whenever another or no revision is selected or the selection's
  qualification is revoked, and otherwise only beside an empty
  `capability_qualifications` (a revoked revision);
- `liveness_unobserved` and `liveness_stale` match the liveness cause.

Anything else is refused with `lattice_wait_reason_inconsistent` or
`lattice_wait_reason_bound`. Inventory v2 carries a reason as a
provider-neutral `{code, subject, owner}` on its definition or worker. The
readiness explanation reports it as a blocker with the exact code, the fact
as subject, `fact:<field>` or `route:<name>` as constraint, and the `owner`.
Kiln's own blockers carry no owner. A reason is an explanation. It is never a
reservation or a dispatch decision.

Kiln also refuses these v3 inputs:

| Input | Refusal |
|---|---|
| Unknown fields, or out-of-vocabulary codes, fields, support values, bases, runtimes, liveness states, slot shapes, evidence classes, provenance, selection kinds or pin statuses | `lattice_inventory_malformed` |
| A definition identity that is not `blake3:` plus 64 lowercase hex characters | `lattice_definition_identity_invalid` |
| A definition name outside 1 to 64 bytes of `[a-z0-9][a-z0-9._:-]*` | `lattice_definition_name_invalid` |
| A declared or qualified set that is empty, has more than 32 names, is unsorted or repeated, or has a name outside the grammar | `lattice_capability_invalid` |
| A support value that disagrees with its value, or `qualified` on any fact but `task_families` | `lattice_fact_support_inconsistent` |
| Profile slots that are partly `unmanaged` | `lattice_revision_slots_inconsistent` |
| An `unknown` slot with another owner than Lattice fixes for it | `lattice_revision_owner_invalid` |
| A revision that is not 64 lowercase hex, a reference or pin identity that is not `blake3:` plus 64 hex, an empty, unsorted or over-bound list (64 revisions, 256 qualification scopes) | `lattice_revision_invalid` |
| A selection whose revision and reference are not a recorded qualification | `lattice_revision_selection_unqualified` |
| Malformed `capability_qualifications` | `lattice_capability_qualification_invalid` |
| `capability_qualifications` other than the unrevoked recorded qualifications of the profile's revision, or a `task_families` support that disagrees with them | `lattice_capability_qualification_inconsistent` |
| A recorded pin on an unmanaged profile or another revision, a live pin whose qualification is not recorded, or an unblocked pin whose recorded qualification is revoked | `lattice_pinned_inconsistent` |
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

With Lattice v3, the definition identity, declared task families and tools,
unavailable capability (a known negative), the revision lifecycle, qualification
references with provenance, worker pins, single-shot liveness and wait reasons
are producer-derived. Exact work that a definition declares support for is no
longer blocked on capability. A family or tool missing from a declared set is
reported as `unsupported_task_family` or `unsupported_agent_tool` with the
exact name. An undeclared set is reported as `agent_task_family_unknown` or
`agent_tool_unknown`, beside Lattice's `capability_undeclared` reason owned by
`configuration`, and never as unsupported. A worker pinned to another revision
of the definition is reported as `worker_pinned_other_revision`.

A managed definition promoted for the needed family (`managed-3` to
`managed-5`) no longer waits on `lattice#25` or `animus#18`: its revision is
selected for new sessions, and the explanation shows the reference with
`fixture` provenance and `task_family_qualification_not_live`. An unmanaged
definition has no lifecycle, so it carries no revision owner, and Kiln reports
`revision_not_selected_for_new_sessions`. Lattice's `revision_not_admitted`
(`lattice#25`) and `qualification_unbound` (`animus#18`) remain where v3 says
so, and a managed fact still unknown keeps its slot owner (preparation
`kiln#49`).

No producer-derived combination can yet be observed eligible. The only
qualification is fixture evidence, no full qualification record (subject,
corpus, protocol, contracts) is published, and the definition's authority is
unpublished. Workers are single-shot and never idle. Their liveness, host
reachability and allocation liveness are unknown (`kiln#49`). Pool capacity,
routing permission and policy are not collected, and fixtures are synthetic.
This is the intended conservative result. The explanation names every exact
blocker. Where Lattice publishes a reason, the blocker carries its exact code
and owner.

## Consumer contract for producers

| Producer | Kiln reads today | Kiln needs before a producer fact can be ready |
|---|---|---|
| Lattice #24 | `lattice.worker-inventory.v3`: definition identity apart from the profile revision; profile revision, adapter, outputs, routes, compute requirement; declared, qualified, unavailable or unknown task families and tools; definition-scoped wait reasons; instance incarnation, attempt, fence, binding, allocation id, served-route service ids, lifecycle, single-shot liveness with cause and retained generation, instance wait reasons | an observed liveness or idle signal for a reusable runtime (Lattice owns none today, so workers are never idle); definition authority for admission; a definition-name field if inventory v2 adds one |
| Lattice #25 | desired, prepared, qualified and selected revision slots, unmanaged, owned unknown or recorded, with rollout generation; capability qualifications; each instance's pin with status and blocked state; `revision_not_admitted` | a published revocation fact for a revision, so that Kiln does not infer it from empty qualifications |
| Lattice #6 | instance `attempt_id`, `fence`, `work_id`; `dispatch_outcome_unknown` and `cancellation_unobserved` reasons | an observed attempt outcome and cancellation, so that these reasons can clear |
| Aspen/Molten #42 | allocation, readback and offer facts through Lattice's DTO | pool capacity, allocatable and reserved amounts per accounting authority, and free offers that are not yet allocated |
| Animus #17/#18 | snapshot readiness, capacity evidence, placement reference; Lattice's serving reasons on workers (`animus#17`); attachment identities, evidence class and provenance through Lattice's capability qualifications, and `qualification_unbound` on definitions (`animus#18`) | routing eligibility for a policy, artifact cache state, and live-provenance qualification with its subject, corpus, protocol and verified contracts |

## Non-claims

- Fixture captures and derived fixtures are not live observations. The committed
  manifests set `synthetic: true`. The Lattice files under
  `fixtures/inventory/producers/lattice/` are verbatim v3 captures.
  `provenance.json` records the Lattice commit (`ed51aee2`), the capture
  command and each file's BLAKE3 hash, and
  an adapter test checks the hashes.
- A qualification reference is evidence, never authority. The captured
  attachment has `fixture` provenance; no live producer has qualified a Lattice
  revision.
- Declared task families and tools are operator statements, not qualification
  or a grant. Task matching beyond exact names belongs to Lattice #26 and
  Kiln #83.
- Collection does not verify producer authenticity. Producer identities are
  compared exactly.
- A displayed explanation is never a reservation or dispatch grant. Owning
  services revalidate at reservation and dispatch.
