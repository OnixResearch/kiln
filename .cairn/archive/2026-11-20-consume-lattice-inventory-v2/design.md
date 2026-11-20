## Context

Kiln #60 maps producer observations into inventory v2
(`kiln.observe.inventory.v2`). The Lattice reader
(`kiln_adapter_lattice::worker_inventory`) mirrors
`lattice.worker-inventory.v1` and lists task families, tools, revisions,
qualifications and liveness as unknown, because v1 publishes none of them.

Lattice commit `9a74e7ef1da4e0ca7610dce0db039b3825a7154d` replaces v1 with
`lattice.worker-inventory.v2` (`docs/worker-inventory.md` there). The profile
(`lattice.worker-profile-facts.v2`) adds `definition { identity, name, basis }`,
`task_families` and `tools` facts, five `revisions` slots owned by Lattice #25
and `wait_reasons`. The instance (`lattice.worker-instance-observation.v2`)
adds `definition_identity`, single-shot `liveness` and `wait_reasons`. Eight
v2 captures of `implementation-attempt inspect` exist. They repeat the seven
v1 scenarios and add one declared-capability scenario. Only the two
`observed_at_epoch_ms` fields vary between capture runs.

## Goals and Success Contract

- v2 is the only accepted Lattice inventory. v1 refuses with
  `lattice_inventory_unsupported_schema`, the existing unsupported-schema error.
- Producer-derived facts become known wherever Lattice publishes them:
  definition identity, declared capability, a known negative for unavailable
  capability, liveness and wait reasons. Anything Lattice leaves unknown stays
  unknown.
- No lifecycle or liveness maps to idle, healthy or executing.
- Wait reasons keep their exact code, subject and owner, and they carry no free
  text.
- No provider type enters `kiln-core`. Existing v2 snapshots keep their
  encoding and projection identity.

## Decisions

### Decision: Clean v2 cutover with a schema probe

**Choice:** The reader first decodes only the envelope, profile and instance
`schema` fields. Any schema other than v2 refuses with
`lattice_inventory_unsupported_schema` before the strict decode. The strict
DTOs mirror v2 field for field and deny unknown fields. The adapter keeps no
v1 DTO. `WORKER_INVENTORY_SCHEMA` becomes `lattice.worker-inventory.v2`, so
collection manifests name the v2 contract.

**Rationale:** A v1 document lacks required v2 fields. A strict decode alone
would report it as malformed instead of unsupported. Lattice no longer emits v1.

### Decision: Definitions keyed by definition identity

**Choice:** A definition's fact id is `profile.definition.identity`
(`blake3:` plus 64 lowercase hex characters) and `profile_revision` stays
`profile.revision`. A worker's `definition` is the instance's
`definition_identity` in the agents join source. `pinned_revision` is the
instance's `profile_revision`. `attempt`, `fence` and the work id stay
unchanged. The definition name must match Lattice's name grammar. The name
and basis are validated but not projected, because inventory v2 has no
definition-name field. Identical repeats collapse. A different fact for one
definition at one revision refuses with `lattice_profile_conflict`. One
definition at two revisions in one agents source refuses with
`lattice_definition_revision_ambiguous`. An instance whose
`definition_identity` differs from its profile's refuses with
`lattice_instance_definition_mismatch`.

**Rationale:** Lattice states that every instance carries its profile's
definition identity, so a mismatch is a contradiction, not an unknown. A v2
definition fact describes one profile revision, and Lattice #25 does not yet
publish which revision is selected for new sessions. Choosing one would invent
a selection, so the ambiguity is refused. A worker pinned to another revision
of a collected definition still joins it, and the explanation reports
`worker_pinned_other_revision`.

### Decision: Declared capability

**Choice:** `task_families` and `tools` are facts of name lists. `declared`
or `qualified` support with a strictly ascending list of 1 to 32 names, each
1 to 64 bytes matching `[a-z0-9][a-z0-9._:-]*`, becomes the known list.
`unavailable` with a null value becomes a known empty list that is not listed
as unknown. `unknown` becomes an empty list with the field in `unknown_fields`.
Any other combination refuses (`lattice_fact_support_inconsistent` or
`lattice_capability_invalid`). The readiness explanation reports a missing
family or tool on a known list as `unsupported_task_family` or
`unsupported_agent_tool`. A family or tool on a list Kiln holds unknown is
reported as `agent_task_family_unknown` or `agent_tool_unknown`, so an unknown
list is never shown as an unsupported one.

**Rationale:** Lattice states that an undeclared set is never `[]` and that
a declared set is complete for its profile. Declared support is an operator
statement, not a qualification. Qualification applicability stays unknown.

### Decision: Revision slots and qualification

**Choice:** The five revision slots must each be `{ support: unknown, owner:
"lattice#25" }`. Any other value refuses with `lattice_revision_unsupported`.
They map to the existing `revisions` fields as `None`, and `revisions` stays
in `unknown_fields`. Their owner is carried by the producer's
`revision_selection_unpublished` wait reason. `qualification_ref` keeps its
v1 mapping (qualified revision and evidence), and `qualifications` stays
unknown.

**Rationale:** A consumer must not treat an unpublished slot as empty or as
the current revision.

### Decision: Liveness into lifecycle

**Choice:** The liveness `runtime` must be `single_shot`. Its `state` is one
of `not_started`, `exited` or `unknown`. A known state has no
`unknown_cause`, and `unknown` has `unobserved` or `stale`. Any other value,
including `idle` or `live`, is malformed. Liveness must agree with the
lifecycle, or the document refuses with `lattice_liveness_inconsistent`:

| Lifecycle | Liveness | Worker lifecycle |
|---|---|---|
| `admitted_not_dispatched` | `not_started` | `reserved` |
| `binding_rejected_before_dispatch` | `not_started` | `stopped` |
| `terminal_observed` | `exited` | `stopped` |
| `dispatch_recorded_outcome_unknown` | `unknown` | `unknown`, with `liveness` in `unknown_fields` |

Repeated observations of one incarnation are ordered by
`retained_generation`, which never decreases for one attempt. The greater
generation wins in any order. Equal generations with different facts refuse
with `lattice_instance_conflict`. Host reachability and allocation liveness
stay owned unknowns. Both observation times must not exceed the collection
time.

**Rationale:** Lattice owns no reusable runtime, idle pool or heartbeat. An
unknown liveness is neither failure nor idle.

### Decision: Provider-neutral wait reasons in the core

**Choice:** `kiln_core::inventory` gains `WaitReason { code, subject, owner }`.
The subject is `WaitSubject::Fact { field }` or `WaitSubject::Route { name }`.
`AgentDefinition` and `WorkerInstance` gain `wait_reasons`, and `Blocker`
gains `owner`. These fields are omitted when empty, and they are defaulted on
read. Admission bounds a fact's reasons to 32, requires public tokens for the
code and subject, and requires owners of the form `name` or `name#number`. It
refuses duplicate reasons. The readiness explanation adds one blocker per
definition reason, and one per worker reason on each of that worker's
candidates. Each such blocker carries the exact code, the fact as its subject,
`fact:<field>` or `route:<name>` as its constraint, and the owner. The
console shows each fact's reasons and each blocker's owner.

The Lattice adapter validates every reason against the published vocabulary
(`lattice_wait_reason_inconsistent` otherwise):

- the owner is fixed per code;
- the code is allowed in its section;
- the subject field fits the code, and a route subject names one of the
  document's `model_routes`;
- there are no more than 32 reasons and no duplicates;
- terminal and rejected instances carry none;
- `capability_undeclared` appears exactly when its field is unknown;
- `liveness_unobserved` and `liveness_stale` match the unknown cause.

Definitions carry only definition-scoped reasons: `capability_undeclared`,
`qualification_unbound` and `revision_selection_unpublished`. A profile's
serving reasons describe one inspection of the serving owner, not the
definition. Serving facts come from the serving producer, as in #60. Workers
carry all of their instance's reasons.

**Rationale:** Codes, owners and subjects are generic: a closed code, the
issue or party that can resolve it, and a field or route the same fact
names. No Lattice type or vocabulary enters the core. Omitting empty lists
keeps existing snapshot encodings and projection identities.

### Decision: Verbatim captures with provenance

**Choice:** The seven v1 files under `fixtures/inventory/producers/lattice/`
are replaced by the eight v2 captures, copied byte for byte.
`fixtures/inventory/producers/lattice/provenance.json` records the Lattice
commit, the capture command and each file's BLAKE3 hash. An adapter test
checks the bytes against it. The collection manifests move their Lattice
window to the captures' clock, `1795208980000` to `1795209040000`, and the
criterion suite evaluates at `1795208985000`. The pilot agents source lists
one revision per definition and adds the declared-capability definition.

**Rationale:** If the published Lattice commit differs, only the provenance
record changes. The captured instances were observed at `1795208972381` to
`1795208972422`, so the earlier window would refuse them as observed after
collection.

## Verification Strategy

- The adapter tests cover positive v2 mapping for every capture and
  provenance hashes. Negative cases cover v1 documents, unknown fields,
  out-of-vocabulary codes, support values and liveness states (`idle` and
  `live`), names over the bounds, unsorted or empty lists, wrong owners,
  sections and subjects, unknown routes, inconsistent liveness, definition
  mismatch, revision ambiguity and generation conflicts.
- The core tests cover wait reason admission, the bounds and owner grammar,
  explanation blockers with owners, unknown-versus-unsupported capability and
  unchanged identity for existing snapshots.
- The criterion suite re-derives every #60 criterion over the v2 fixtures. It
  adds declared capability that satisfies exact work, an unavailable tool as a
  known negative, an undeclared capability as unknown and producer reasons
  with owners.
- The node UI tests cover the rendering of owners and wait reasons.
- Repository gates run once at the end.

## Risks and Trade-offs

- Lattice #25 is not implemented, so a collector cannot list two revisions of
  one definition in one agents source. Other revisions remain visible through
  workers that pin them.
- Producer-derived combinations still cannot be observed eligible. Revision
  selection (Lattice #25), qualification (Animus #18), definition authority,
  pool capacity (#42), routing permission and policy (Animus #17/#18) and an
  idle worker are unpublished, and fixtures are synthetic. The difference is
  that capability checks pass on declared facts, and the remaining blockers
  name exact producer reasons and owners.
- Captures are synthetic fixture outputs of Lattice commit `9a74e7ef` (published
  on Lattice main), not live observations.
