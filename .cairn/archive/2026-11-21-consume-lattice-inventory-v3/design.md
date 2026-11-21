## Context

Kiln #60 maps producer observations into inventory v2
(`kiln.observe.inventory.v2`). The Lattice reader
(`kiln_adapter_lattice::worker_inventory`) mirrors
`lattice.worker-inventory.v2` (change `consume-lattice-inventory-v2`, archived
as `.cairn/archive/2026-11-20-consume-lattice-inventory-v2`). It requires the
five revision slots to be unknown with owner `lattice#25`, keeps `revisions`
unknown, maps the profile's single `qualification_ref` into the revision
evidence, and refuses one definition at two revisions in one agents source.

Lattice #25 replaces v2 with `lattice.worker-inventory.v3`. The changes that
matter to Kiln:

- The profile drops `qualification_ref` and gains `capability_qualifications`:
  `{ task_family, qualification_ref, evidence_class, provenance }` for every
  unrevoked qualification of the profile's own revision. It is empty without a
  lifecycle and when the revision is revoked. `task_families` is `qualified`
  only when every declared family has one.
- Four profile slots (`desired`, `prepared`, `qualified`,
  `selected_for_new_sessions`) describe the definition, and the instance has
  `pinned`. Each slot is `{support: unmanaged}`, `{support: unknown, owner}`
  or `{support: recorded, value}`.
- `revision_selection_unpublished` is gone. `revision_not_admitted`
  (`lattice#25`) marks a managed profile whose revision would be refused for a
  new session. `qualification_unbound` now names `capability_qualifications`.
- A new instance lifecycle, `revision_blocked_before_dispatch`.
- The revision no longer frames per-attempt values. In v2 each attempt had its
  own revision, because the task prompt and base snapshot were hashed.

Sixteen v3 captures exist: the eight v2 scenarios, all unmanaged, and seven
managed lifecycle scenarios (`managed-1` to `managed-7`, two files for
`managed-6`). Their promotion uses an Animus #18 attachment with `fixture`
provenance. The captures come from Lattice `ed51aee2` and are bound by
BLAKE3.

## Goals and Success Contract

- v3 is the only accepted Lattice inventory. v2 refuses with
  `lattice_inventory_unsupported_schema`.
- Every published revision fact maps onto the definition or the worker.
  `unmanaged`, `unknown` and `recorded` stay distinguishable in Kiln.
- Several revisions of one definition may coexist when Lattice records which
  one is selected. Contradictory facts still refuse with a named reason.
- A task family is shown as qualified only with a current reference, and its
  provenance is always shown. A `fixture` reference never counts as live
  evidence.
- Wait reasons drop `lattice#25` where v3 publishes the fact and keep an
  owner where v3 still leaves it unknown.
- No provider type enters `kiln-core`. Existing snapshots keep their encoding
  and projection identity.

## Decisions

### Decision: Clean v3 cutover

**Choice:** The schema probe accepts only `lattice.worker-inventory.v3`,
`lattice.worker-profile-facts.v3` and `lattice.worker-instance-observation.v3`.
Any other label, including v2, refuses with
`lattice_inventory_unsupported_schema` before the strict decode. The DTOs
mirror v3 field for field and deny unknown fields. No v2 DTO remains.

**Rationale:** The same rule as the v1 to v2 cutover. v2 revisions were
per-attempt, so they must never be compared with v3 revisions.

### Decision: Revision slots with three shapes

**Choice:** A slot decodes as `unmanaged`, `unknown` with its owner, or
`recorded` with its value. The four profile slots are either all `unmanaged`
or none (`lattice_revision_slots_inconsistent`). Unknown owners are fixed per
slot: `desired` and `selected_for_new_sessions` `lattice#25`, `prepared`
`kiln#49`, `qualified` `animus#18`, `pinned` `lattice#25`
(`lattice_revision_owner_invalid`). Revisions are 64 lowercase hex characters,
references `blake3:` plus 64 lowercase hex, lists non-empty, strictly sorted
and bounded by Lattice's 64 revisions and 256 qualification scopes, and
evidence classes and provenance from the closed vocabularies
(`lattice_revision_invalid` or malformed).

The definition maps as follows:

| Kiln | From Lattice |
|---|---|
| `revisions.management` | `unmanaged` when every slot is `unmanaged`, else `managed` |
| `revisions.desired` | recorded `desired.revision` |
| `revisions.prepared` | the profile's revision when the recorded `prepared` list holds it |
| `revisions.qualified` | the profile's revision when `capability_qualifications` is not empty |
| `revisions.selected_for_new_sessions` | recorded `selected_for_new_sessions.revision` |
| `revisions.generation` | the selection's `rollout_generation`; 0 without a selection |
| `revisions.evidence` | the references in `capability_qualifications` |
| `task_family_qualifications` | `capability_qualifications`: task family, reference, evidence class, provenance |
| `meta.unknown_fields` | `revisions.<slot>` for each `unknown` slot, and `qualifications` |
| `wait_reasons` | definition-scoped producer reasons, plus `revision_fact_unknown` on `revisions.<slot>` with the slot's owner for each `unknown` slot |

The worker maps as follows:

| Kiln | From Lattice |
|---|---|
| `pinned_revision` | `profile_revision`, which a recorded pin must equal |
| `pin` | `{state: unmanaged}` or `{state: recorded, qualification, rollout_generation, identity, status, blocked}` from `pinned` |
| `meta.unknown_fields` | `pinned` when `pinned` is `unknown` |
| `wait_reasons` | the instance's reasons, plus `revision_fact_unknown` on `pinned` (`lattice#25`) when `unknown` |

An `unmanaged` slot is neither empty nor unknown: it is not listed as unknown
and carries no reason. An absent `pin` with `pinned` unknown means unknown; an
absent `pin` without it means the producer publishes no pin.

**Rationale:** Kiln's revision slots are single revisions, so the two list
slots answer for the projected revision, which is the one readiness compares.
The full lists stay in the producer document. A revoked revision has no
current qualification, so `capability_qualifications`, not the recorded
attachment list, decides `qualified`.

### Decision: Selected-versus-pinned replaces the two-revision refusal

**Choice:** All documents of one definition in one source must carry the same
revision lifecycle. Two different recorded selections refuse with
`lattice_definition_selection_conflict`, and any other difference with
`lattice_definition_lifecycle_conflict`, in agents and workers sources alike.
An agents source projects one definition: its only revision, or, when several
appear, the recorded selected revision. Several revisions without a recorded
selection, or with a selection that none of them is, refuse with
`lattice_definition_revision_ambiguous`. Profiles of one revision must agree
(`lattice_profile_conflict`). Workers pinned to other revisions stay visible,
and the explanation reports `worker_pinned_other_revision`.

Within one document, Kiln refuses contradictions:

- a selection whose revision and reference are not a recorded qualification
  (`lattice_revision_selection_unqualified`);
- `capability_qualifications` that are not exactly the unrevoked recorded
  qualifications of the profile's revision, or a `task_families` support that
  disagrees with them (`lattice_capability_qualification_inconsistent`);
- a recorded pin on an unmanaged profile, on another revision, a live pin
  whose qualification is not recorded, or an unblocked pin whose recorded
  qualification is revoked (`lattice_pinned_inconsistent`);
- `qualification_unbound` that does not appear exactly when `task_families`
  is not `qualified`, and `revision_not_admitted` on an unmanaged profile,
  missing when another revision is selected or the selection's qualification
  is revoked, or present beside current qualifications of the selected
  revision (`lattice_wait_reason_inconsistent`).

**Rationale:** v3 says which revision is selected and which one each session
pins, so older revisions legitimately coexist. Two selections for one
definition, or a selection without its qualification, cannot both be true.

### Decision: Qualification references with provenance in readiness

**Choice:** `kiln_core::inventory` gains `TaskFamilyQualification { task_family,
reference, evidence_class, provenance }` with `EvidenceProvenance` `fixture` or
`live`, listed on `AgentDefinition.task_family_qualifications` (sorted, one
per family). The readiness explanation gains `task_family_qualification`: the
needed family's entry, only when the definition is at the needed revision and
its fact is neither missing, stale nor in the future. A shown `fixture`
reference adds Kiln's blocker `task_family_qualification_not_live` with the
reference as constraint. The full `qualifications` record (subject, corpus,
protocol, contracts) stays unpublished, so `qualification_missing_stale_or_mismatched`
still applies.

**Rationale:** A reference is evidence, never authority, and Lattice publishes
provenance so that consumers can tell fixture evidence apart. Kiln never
presents fixture evidence as live.

### Decision: Provider-neutral revision management and pins

**Choice:** `RevisionState` gains `management: Option<RevisionManagement>`
(`unmanaged` or `managed`). `WorkerInstance` gains `pin: Option<RevisionPin>`,
`unmanaged` or `recorded { qualification, rollout_generation, identity,
status, blocked }` with `PinStatus` `live`, `closed` or `released`. A recorded
blocked pin adds Kiln's candidate blocker `worker_pin_blocked` with the pin
identity as constraint. New fields are omitted when empty and defaulted on
read; admission validates their tokens, bounds and order.

**Rationale:** `unmanaged` must stay distinct from `unknown` and from a
recorded pin. The names are generic revision-lifecycle terms, not Lattice
types.

### Decision: Owner-attributed reasons only where facts are missing

**Choice:** The adapter drops `revision_selection_unpublished`. For each
`unknown` slot it adds `revision_fact_unknown` with the slot as subject and the
slot's published owner. Lattice's `revision_not_admitted` and
`qualification_unbound` pass through with their owners.

**Rationale:** Preparation (`kiln#49`), qualification (`animus#18`) and
selection (`lattice#25`) remain owner-attributed exactly while v3 leaves them
unknown.

### Decision: Verbatim captures with provenance

**Choice:** The eight v2 files under `fixtures/inventory/producers/lattice/`
are replaced by the 16 v3 captures, byte for byte. `provenance.json` records
the contract, the Lattice commit (`ed51aee2`), the capture command and each
file's BLAKE3 hash. The adapter test
checks the hashes, not the commit. The collection windows move to
`1795237890000` to `1795237950000`, and the criterion suite evaluates at
`1795237895000`.

**Rationale:** The captured instances were observed at `1795237881806` to
`1795237881966`, after the v2 window.

## Verification Strategy

- Adapter tests map every capture: unmanaged, unknown and recorded slots,
  the managed lifecycle `managed-1` to `managed-7`, a revoked qualification, a
  pin on a non-selected revision, `fixture` and `live` provenance, and the
  provenance hashes. Negative cases cover v2 documents, mixed or misowned
  slots, malformed revisions and references, two selected revisions,
  selections without qualifications, inconsistent capability qualifications,
  pins and reasons.
- Core tests cover admission of the new fields, the explanation's
  task-family qualification and its provenance blocker, the pin blocker and
  unchanged identity for existing snapshots.
- The criterion suite re-derives #60 over the v3 fixtures and adds a managed
  definition whose fixture qualification is shown and not counted as live.
- The node UI tests cover the rendering of management, provenance and pins.
- Repository gates run once at the end.

## Risks and Trade-offs

- Kiln's single `prepared` and `qualified` slots answer for the projected
  revision; other revisions in Lattice's lists are not projected.
- Revocation of a revision is not a v3 slot; Kiln sees it as an empty
  `capability_qualifications`, `revision_not_admitted` and blocked pins.
- One source with two observations of one incarnation at one retained
  generation, before and after a revocation, refuses as
  `lattice_instance_conflict`.
- Producer-derived combinations still cannot be observed eligible. The only
  qualification is fixture evidence, and authority, pool capacity, routing
  permission, policy and an idle worker are unpublished. Captures are
  synthetic.
