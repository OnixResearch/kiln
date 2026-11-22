## Context

The Cairn change detail (`kiln.cairn-workspace-detail.v3`) projects an
**Agents and revisions** section from Kiln's admitted resource inventory
(`cairn_workspace::agents::project`). Its `workflow_revisions` is a constant:
`selected_for_new_sessions` and `pinned`, both `unpublished`, owner
`lattice#25`.

Kiln #60 obtains Lattice worker inventory v3 this way:

1. A collecting shell writes a `kiln.inventory.collection-set.v1` manifest
   naming, per source, its kind, contract, window and documents.
2. `inventory_collection` reads exactly those regular files under byte bounds
   and dispatches each source by `(kind, contract)` to an adapter.
3. `kiln_adapter_lattice::worker_inventory` probes the schema labels, decodes
   field-for-field DTOs that deny unknown fields, validates them and maps
   them into `kiln_core::inventory` facts.
4. `compose_snapshot` joins the fragments; `admit_snapshot` checks them
   against an inventory profile (`kiln.observe.inventory.v2`) whose sources
   allowlist source, producer, scope and kind; `project_inventory` derives
   source and fact states.
5. The detail route reads the admitted `{profile, snapshot}` input with
   `read_inventory` and hands the projection to the section.

Fixtures are verbatim Lattice captures with a `provenance.json` that records
the commit, the command and each file's BLAKE3 hash.

Lattice `7fb6dc47` publishes `lattice.workflow-agent-revision-slots.v1`
(`lattice_core::agent_revision::workflow::publication::slots`, printed by
`agent-revision inspect` as `outcome.workflow_slots`):

| Field | Meaning |
|---|---|
| `schema`, `definition`, `name` | The contract, the workflow-agent definition and its pairing name |
| `desired`, `last_usable` | An entry or `null` |
| `selected_for_new_sessions` | `{bundle: entry, rollout_generation, kind, qualification_ref}` or `null` |
| `pinned[]` | `{session, bundle: entry, status, is_blocked}`, sorted by session |
| entry | `{revision, workflow: {compiler_revision, compiler_artifact_identity, plan_identity, graph_identity, selection_identity, obligations_identity, policy_identity}, worker: {definition, definition_name, revision, bundle_identity}}` |

A workflow-agent definition is named by its pairing name and its worker
definition, and a workflow revision is a content identity over the plan and
the worker bundle.

## Goals and Success Contract

- The slots document is read only through the existing mechanism: manifest,
  bounded read, schema-probed strict adapter, profile admission, projection.
- The workflow slot of the detail renders from the captured `7fb6dc47`
  fixtures: workflow revision and identities, the paired worker revision,
  pin state, and `recorded`, `not_recorded` or `unknown`.
- A missing, refused or schema-invalid slots document leaves the rest of the
  inventory and of the detail intact and marks only the workflow slot
  `unavailable` with its refusal code, or `not_configured`.
- No guidance, context, source, session, grant or name reaches Kiln's facts,
  the detail or the page.

## Decisions

### Decision: workflow revision slots are an inventory kind

**Choice:** `InventoryKind` gains `workflow_revisions`, the Nickel profile
contract admits it, and `InventorySnapshot` and `InventoryFragment` gain
`workflow_revisions: Vec<WorkflowRevisionSlots>`, omitted when empty. The
collection shell maps contract `lattice.workflow-agent-revision-slots.v1`
into that kind only. Admission checks the source's kind like every other
fact, and `project_inventory` gives each fact a state.

**Rationale:** the same profile, freshness, principal and projection identity
apply, and the detail's view identity already binds the inventory projection.
Earlier snapshots keep their encoding and identity.

### Decision: a provider-neutral, identity-only model

**Choice:** `kiln_core::inventory::WorkflowRevisionSlots` has the fact's
`meta`, `revisions` (each named revision once, strictly ascending:
`revision`, `workflow` with `compiler_revision`, `compiler_artifact`, `plan`,
`graph`, `selection`, `obligations`, `policy`, and `worker` with the
definition as a reference into the agents join source, `revision` and
`bundle`), `desired`, `last_usable`, `selected_for_new_sessions`
(`revision`, `rollout_generation`, `kind` `promotion` or `rollback`,
`qualification`) and `pinned` (`revision`, `status`, `blocked`, `sessions`),
strictly ascending by revision, status and blocked state. Admission requires
public tokens, at most 64 revisions and 1024 pinned sessions (Lattice's
`MAX_CANDIDATES` and `MAX_SESSIONS`), every named revision listed and every
listed revision named.

**Rationale:** a revision appears in several slots; listing it once keeps one
binding per revision. Counting pins keeps session identities out without
losing how many sessions a revision serves.

### Decision: the adapter refuses what Lattice would not publish

**Choice:** `map_workflow_revision_slots` refuses, before any fact:

| Input | Refusal |
|---|---|
| More than 256 KiB | `lattice_workflow_slots_byte_bound` |
| Another schema label | `lattice_workflow_slots_unsupported_schema` |
| Not JSON, unknown or missing fields, out-of-vocabulary kinds or statuses | `lattice_workflow_slots_malformed` |
| A definition that is not `blake3:` plus 64 hex | `lattice_workflow_definition_invalid` |
| A name outside Lattice's token | `lattice_workflow_name_invalid` |
| A revision that is not 64 lowercase hex | `lattice_workflow_revision_invalid` |
| A plan or cohort identity that is not `b3:` plus 64 hex, a graph that is not `blake3:` plus 64 hex, or a compiler that is not a public token | `lattice_workflow_identity_invalid` |
| A worker definition or bundle that is not `blake3:` plus 64 hex, a worker revision that is not 64 hex, or a bad worker name | `lattice_workflow_worker_invalid` |
| Rollout generation 0 or a qualification that is not `blake3:` plus 64 hex | `lattice_workflow_selection_invalid` |
| More than 1024 pins | `lattice_workflow_pin_bound` |
| Sessions not strictly ascending, or not tokens | `lattice_workflow_pin_invalid` |
| A closed pin that is not blocked | `lattice_workflow_pin_inconsistent` |
| Two worker definitions in one document | `lattice_workflow_worker_binding_mismatch` |
| One revision with two bindings | `lattice_workflow_revision_conflict` |
| Two different documents for one definition in one source | `lattice_workflow_definition_conflict` |

Names and sessions are validated and dropped. Identical repeats collapse.

**Rationale:** the definition frames the worker definition and the revision
frames the binding, so either mismatch is a corrupt or substituted document.
A guidance, context or source identity is an unknown field and is refused,
never copied.

### Decision: refused slots make only their own source unavailable

**Choice:** for a `workflow_revisions` source with the slots contract, a
document read failure, byte bound or adapter refusal becomes that source's
`unavailable_reason` with no facts. Manifest defects (path, contract,
unavailable source with documents) still refuse the collection, and every
other kind keeps refusing the whole collection.

**Rationale:** no readiness decision reads workflow revisions. Failing the
whole inventory would hide the agents and workers for a display-only fact,
and the source state already carries an exact reason to the detail.

### Decision: the detail projects workflow revisions with their own boundary

**Choice:** the detail becomes `kiln.cairn-workspace-detail.v4`.
`agents.workflow_revisions` is `{source, reason, sources, definitions}`:

- inventory not configured or unavailable: the same state and reason as the
  section;
- no `workflow_revisions` source in the profile: `not_configured`;
- configured sources none of which was observed (`missing`, `out_of_order`
  or `unavailable`): `unavailable` with the first source reason, else
  `workflow_revisions_source_missing`, `..._out_of_order` or
  `..._unavailable`;
- otherwise `admitted` with one view per fact: the fact's observation state;
  `desired`, `last_usable` and `selected_for_new_sessions` as `recorded` with
  the revision view, `not_recorded`, or `unknown` when the fact lists the slot
  in `unknown_fields`; the selection's rollout generation, kind and
  qualification; `pinned_state` (`recorded`, `not_recorded` or `unknown`) and
  each pin group with its revision view, status, blocked state, session count
  and `matches_selected`, `differs_from_selected` or `no_selection`.

A revision view copies the revision, the seven workflow identities, the
worker definition, revision and bundle, and `observed`: `profile_revision`
when the inventory projects that worker revision for the definition,
`other_revision`, or `definition_not_observed`. Workflow sources are listed
apart from agents and workers sources.

**Rationale:** the shape changes, so the schema moves and the console refuses
v3. Workflow lifecycles are always managed, so an empty slot is
`not_recorded`; no producer names an owner for an unknown workflow slot, so
none is shown. Comparing the paired worker revision with the projected one
shows the pairing without inferring which worker runs a workflow.

### Decision: captures from Lattice's own commands

**Choice:** a capture test, added only to a detached worktree of Lattice
`7fb6dc47` and kept in this change's evidence, drives one workflow-agent
lifecycle with Lattice's composition, `desire`, `run_command_with_ports`
(`Prepare`, `Promote`) with the fixture ports, and `seam::admit`/`close`,
then reads each state with the `lattice agent-revision inspect` binary and
writes `outcome.workflow_slots`. Four scenarios: desired, promoted, a live
pin, and a second revision promoted while the first revision's session is
closed. `provenance.json` records the commit, command, program hash and file
hashes; the adapter test checks the hashes.

**Rationale:** the standalone CLI refuses promotion without real ports, so the
fixture ports the Lattice suites use are the only way to reach a selection
and pins; `inspect` itself is the binary.

## Risks / Trade-offs

- Collection isolation differs for this one display-only kind; the refusal
  code is still visible as the source reason.
- The paired worker comparison uses the definition's projected revision, not
  every revision a worker pins.
- Captures are synthetic and from fixture ports; no live Lattice store was
  read.
- A workflow definition is not yet bound to a Cairn unit (#83); every
  workflow definition in the inventory is listed.

## Verification Strategy

- Adapter tests over the four captures and derived negatives.
- Core admission and identity tests.
- Collection tests: the pilot collects the capture; wrong schema, extra
  field, oversized, mismatched worker binding and missing document each make
  only the workflow source unavailable; manifest defects still refuse.
- Projection, detail and route tests; Node UI tests; repository gates.
