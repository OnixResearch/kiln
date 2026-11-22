## Context

The Cairn workspace (`cairn_workspace.rs`, archived changes
`add-cairn-workspace` and `rework-cairn-changes-page`) projects one Cairn
change from one bounded canonical snapshot of `--implementation-work`. Its
detail is `kiln.cairn-workspace-detail.v2` with a BLAKE3 view identity.

Resource inventory is a separate input (`--inventory`), an admitted
`{profile, snapshot}` document. `crate::inventory::read_inventory(path,
principal, now)` refuses another principal and returns the
`kiln.observe.inventory.v2` projection: per-source and per-fact observation
states and the admitted snapshot. Since `consume-lattice-inventory-v3`, an
`AgentDefinition` carries `revisions` (with `management`), `wait_reasons` and
`task_family_qualifications`, and a `WorkerInstance` carries
`pinned_revision`, `pin` and `wait_reasons`. A slot the producer leaves
unknown is listed in `meta.unknown_fields` as `revisions.<slot>` (or `pinned`)
and carries a `revision_fact_unknown` wait reason with the slot's owner.

The selected-campaign projection already composes an optional inventory beside
its canonical facts with an independent failure boundary
(`compose_resource_inventory`).

## Goals and Success Contract

- A Cairn change's detail shows per-definition revision facts, per-worker pins
  and blocked state, task-family qualifications and owner-attributed wait
  reasons, read only from the admitted inventory projection.
- `unmanaged`, `unknown` with owner, `recorded`, and a managed slot with no
  value stay distinct in data and on screen.
- A `fixture` reference is never shown or counted as live evidence.
- Workflow revisions appear as their own unpublished slot, owned by
  `lattice#25`, apart from worker revisions.
- No session, workspace, execution or grant reference, model-service binding,
  full qualification record, preparation failure, or other work's attempt
  appears.
- An unconfigured or failing inventory never fails the detail and never reads
  as an empty fleet.

## Decisions

### Decision: a pure section over the admitted inventory projection

**Choice:** `cairn_workspace::agents::project(observation, attempts)` takes an
`InventoryObservation` (`NotConfigured`, `Unavailable(code)` or
`Admitted(InventoryProjection)`) and the set of `(attempt identity, fence)`
pairs of the change's attempts, and returns `AgentRevisions`. It reads no
storage, clock or environment. `cairn_workspace::detail` takes the observation
and places the section in the detail before hashing the view identity, so the
identity binds the inventory projection identity it was computed from.

**Rationale:** the inventory projection has already applied source policy,
principal, freshness and admission. Reading producer documents again would
bypass them.

### Decision: one slot vocabulary for definitions, workers and workflows

**Choice:** a slot is `{ state, value, owner }`:

| State | When |
| --- | --- |
| `unknown` | the slot (or the whole `revisions` field) is listed in `unknown_fields`; `owner` is the owner of the matching `revision_fact_unknown` reason, or absent when none names it |
| `recorded` | a value is present |
| `unmanaged` | the definition's management is `unmanaged` |
| `not_recorded` | the definition is `managed` and the slot has no value for the projected revision |
| `unpublished` | the producer does not say whether a lifecycle exists and has no value |

A worker's pin uses the same states: `recorded` adds the qualification
reference, rollout generation, pin identity, status and `blocked`; `unknown`
comes from `pinned` in `unknown_fields` with the owner of its reason; an
absent pin that is not unknown is `unpublished`. Workflow revisions are
`{ selected_for_new_sessions, pinned }`, both `unpublished` with owner
`lattice#25`, because no producer publishes them.

**Rationale:** `unmanaged` is a known fact about the lifecycle, `unknown` is a
known gap with an owner, and `unpublished` is a gap no producer has claimed.
Collapsing any two would hide who can resolve the gap.

### Decision: selected versus pinned per worker

**Choice:** each worker reports `selection`:
`matches_selected` when its pinned revision equals its definition's recorded
selection, `differs_from_selected` when it does not, `no_selection` when the
definition records no selection, and `definition_not_observed` when the
definition is not in the snapshot.

**Rationale:** Lattice #25 asks for selected and pinned revisions side by side.
A worker pinned to an older revision is legitimate and stays visible.

### Decision: provenance stays attached to each qualification

**Choice:** each declared task family lists its qualification reference with
evidence class, provenance and `counts_as_live`, which is true only for `live`
provenance. A family with no reference is `unqualified`. A reference for an
undeclared family stays listed with `declared: false`. Capability lists are
`declared`, `none` (known empty) or `unknown` (listed in `unknown_fields`).

**Rationale:** the reader of #60 already blocks readiness on
`task_family_qualification_not_live`; the workspace must not undo that by
showing a fixture reference as live.

### Decision: an explicit allow-list of copied fields

**Choice:** the section copies only fact references, observation states,
profile and pinned revisions, revision slots and generations, capability
names, task-family qualification references, lifecycle, pin facts and wait
reasons (`code`, `fact:<field>` or `route:<name>`, `owner`). A worker's
attempt identity and fence are copied only when they equal an attempt of this
change (`work: this_change`); otherwise the worker reads `other_work` or
`no_attempt` without the identity.

**Rationale:** Lattice #24 and #25 forbid commands, prompts, secrets, paths and
private runtime handles. The admitted inventory contains none of the first
four, but sessions, workspaces, execution allocations, grants and
model-service bindings are runtime handles, and other work's attempts are not
this change's facts. An allow-list keeps future inventory fields out until
someone decides to show them.

### Decision: the detail route reads inventory lazily with its own failure boundary

**Choice:** `cairn_workspace::http::response` takes a callback that yields the
`InventoryObservation` and calls it only for the detail route. The service
passes a callback over `--inventory`, the request principal and the host clock,
mapping a missing input to `NotConfigured` and every refusal, including a
clock failure, to `Unavailable(code)`. The exploration workspace passes its
synthetic inventory projection.

**Rationale:** the index never needs inventory, and a failing inventory must
leave the canonical Cairn facts readable, as for the campaign projection.

### Decision: detail schema v3 and a page section

**Choice:** the detail schema becomes `kiln.cairn-workspace-detail.v3`; the
console refuses v2 details. The page adds **Agents and revisions** after
Campaigns: the inventory source state, a separate **Workflow revisions** slot,
one card per definition (slots, qualifications, capability, wait reasons) and
one row per worker (pin, selection, work binding, wait reasons). Unknown
values and fields the section does not define are never rendered.

**Rationale:** the section is a new required field, and a renderer that saw a
v2 document would show no agents without saying why.

## Risks / Trade-offs

- The section lists every definition and worker in the admitted inventory, not
  only those a unit's work needs: Kiln does not yet bind a Cairn unit to an
  agent definition (#83 supplies typed requirements). Workers bound to this
  change's attempts are marked and listed first.
- Every authenticated viewer and operator sees the same projection, as for the
  inventory route. Per-holder disclosure (#69/#70/#75/#76/#79) remains open.
- Kiln's `prepared` and `qualified` slots answer for the projected revision
  only, as in the #60 reader.
- The view identity now changes when the inventory projection changes, so two
  reads of an unchanged canonical store can differ.

## Verification Strategy

- Projection tests over the Lattice v3 producer fixtures: unmanaged pilot
  captures, `managed-1` (unknown slots with owners `kiln#49`, `animus#18`,
  `lattice#25`), `managed-3` with `managed-4`/`managed-5` (recorded selection,
  fixture provenance, recorded pins), and `managed-7` with `managed-6` (revoked:
  `not_recorded` qualification, `revision_not_admitted`, blocked pins). Negative
  cases: injected restricted values never appear, other work's attempts are
  withheld, fixture is never live, a worker on another revision differs from
  the selection, and unconfigured or unavailable inventory keeps its reason.
- Detail tests: the section and its identity binding; route tests for the lazy
  callback.
- Node UI tests: slot words, provenance, pins, workflow slot, source states and
  ignored unknown fields.
