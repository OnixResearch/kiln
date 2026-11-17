## Context

`cairn_workspace.rs` projects one bounded canonical snapshot into an index of changes and a per-change detail:
- units with five facets (readiness, execution, evidence, integration, lifecycle);
- attempts with applicability, verification, publication and lifecycle records;
- campaign memberships;
- decisions;
- structured blockers.

`cairn.js` renders both as a narrow list beside a long sheet of definition lists. The campaign view (`inventory.js`) already renders the campaign inbox. It offers answer forms only for listed actions, and its command runner sends the inbox store version and a fresh request ID, reuses the ID on retry and reloads after a conflict (`kiln.implementation.campaign.controls.scenario.console`). The console already has segmented controls, chips, ruled tables that stack, side metrics, a run DAG (`.dag-*`) and a Workers register-to-detail convention.

## Goals and Success Contract

The register gives three things at a glance:
- which changes need attention;
- where each change stands;
- what blocks it.

Opening a change answers four questions in order:
1. What blocks it?
2. Who owns the next step, and where is it resolved?
3. Where is each unit?
4. What happened over time, and how do the units depend on each other?

Exact identities stay reachable but out of the reading path. Decisions can be answered in place through the existing owner path.

False completion includes any of the following:
- a lane that hides a stop or advances on historical evidence or on an unrecognized value;
- a board position presented as acceptance or lifecycle authority;
- a filter or late response that substitutes a different change or unit;
- an answer form for a decision the inbox does not admit, or for another change's member;
- any campaign command other than resolving or issuing a decision;
- a graph that loops on a cycle or drops a not-current dependency.

## Decisions

### Decision: Lanes are projection facts derived from the facets

**Choice:** `cairn_workspace.rs` gives each unit a `lane` in display order: `attention`, `awaiting`, `ready`, `in_progress`, `verified`, `integrated`, `recorded`. The rule, in precedence order:

1. `attention` when any of these holds:
   - readiness is `blocked` or `unknown`;
   - execution is anything other than `not_started`, `claimed`, `executing`, `candidate_produced`, `verifying` or `verified`;
   - evidence is `current_failed`.
2. Otherwise `awaiting` when readiness is `not_admitted`.
3. Otherwise `recorded` when lifecycle is `receipt_observed`.
4. Otherwise `integrated` when integration is `integrated` or `source_reconciled`.
5. Otherwise `verified` when evidence is `current_verified`.
6. Otherwise `ready` when execution is `not_started`.
7. Otherwise `in_progress`.

A change's lane is `attention` if any unit is stopped. Otherwise it is the earliest lane among its units, or `awaiting` when no unit is named. The index entry and the detail overview carry `lane`, `lanes` (unit counts for every lane) and `open_decisions` (decisions that are `unissued` or `outstanding`). The index entry also carries `first_blocker`. Both schemas move to `v2`, and the console refuses `v1`.

**Rationale:** the index must not ship every unit to place a card, and one owner keeps register, board and detail consistent. The facets stay in the detail and are always shown beside the lane. The lane is a position, not a status that replaces them. Historical evidence counts only through `current_verified`, so it cannot advance a unit. Change-level campaign blockers (holds, stale bindings, pending effects) do not move lanes. They are shown on every row and card, and they have their own filter, so a review wait never reads as a failure.

### Decision: Blockers rank by condition and name their target

**Choice:** blockers sort by a fixed rank:
1. `dispatch_recovery`
2. `dependency_cycle`
3. `no_acceptance_obligations`
4. `missing_dependency`
5. `stale_work_binding`
6. `pending_effect`
7. `campaign_hold`

Ties sort by subject and observation. An unrecognized condition cannot arise from the projection. Each blocker adds `subject_kind` (`unit` or `member`) and `campaign_id` (present for member blockers). `first_blocker` is the highest-ranked blocker.

**Rationale:** an unknown outcome must be reconciled before anything else can proceed, so it outranks a review wait. The page links a subject and its campaign from structured fields, never from parsed observation or reason text.

### Decision: Dependents come from the current ledger

**Choice:** each current `WorkView` adds `dependents`: the current work that names it as a dependency, with the native identities of that work. The fields are the same as `dependencies`, whose `state` is always `current`.

**Rationale:** a prerequisite's page can show what waits on it, even across changes, without another read.

### Decision: A register first, with list and board over one row set

**Choice:**

Layout:
- With no change selected, the page is a full-width register.
- A toolbar holds the search field, lane chips with counts (single choice), "Has blockers" and "Has open decisions" toggle chips, Clear filters, and a List/Board segmented control.
- The list is a ruled stacking table. Its columns are:
  - Change: name, projects and short repository identity;
  - Stage: lane badge and a bar of unit counts per lane;
  - Next step: first blocker condition, subject and owner, or "No blockers";
  - Decisions: open decisions;
  - Attempts: count and active.
- The board has one column per lane with a count. Its cards hold the same facts.

Order and filtering:
- Rows order by lane rank, then blockers (descending), open decisions (descending), change name and repository identity. The order never depends on arrival.
- Filtering is local and immediate. URL keys:
  - `cairn_q`, bounded to 256 characters;
  - `cairn_lane`, one known lane;
  - `cairn_blocked=1`;
  - `cairn_decisions=1`;
  - `cairn_view=board`.
- Typing commits `cairn_q` after 350 ms. A self-applied commit is not re-read or redrawn. Chip and view changes redraw from the cached model without a read.
- A count line reports the number shown of the total.

Keyboard: the arrow keys move between visible change links. In the board, left and right move between columns.

**Rationale:** this follows the Workers rework and the existing chip, segmented-control and table components. Lanes give the board a meaning even though a change spans units in several stages, because the unit bar shows that spread.

### Decision: A change reads as next steps, units, dependencies, campaigns, then identities

**Choice:** a selection replaces the register with a full-width detail.

Navigation:
- "All changes" clears only the selection and returns focus to the change's register link.
- Previous and next step through the filtered register order.
- "Browse changes" is a disclosure that holds the register with its filters intact.

Header:
- the change name;
- its lane badge and unit bar;
- projects;
- readouts for units, attempts (active), campaigns, blockers and open decisions.

Sections, in order:
1. **Next steps.**
   - Each ranked blocker shows its condition in words with its meaning, then its subject. A unit subject links to its unit row and expands it; a member subject links to the member card.
   - It then shows "operation by owner", the display-only detail, and a short observation that keeps the exact value in its title.
   - A member blocker links to its campaign view.
   - An empty list says this is not readiness on its own.
   - Open decisions follow (see the inline-decisions decision).
2. **Units.**
   - A matrix: one row per unit, one column per facet, each cell in its own words and tone.
   - The unit cell is a button that shows the lane, carries `aria-expanded`, and toggles a detail row bound to `cairn_unit`. One unit is open at a time, and the arrow keys move between unit buttons.
   - The detail row shows:
     - current work: title, outcome, revision, priority, terminal policy and acceptance obligations;
     - dependencies and dependents. Links open a unit of this change or the named change with the unit selected. A not-current dependency stays visible with its identity.
     - superseded versions;
     - an attempt timeline from oldest to newest. Each entry shows applicability, phase, times, dispatch, unknown outcome, verification (with a link to its exact run), publication and lifecycle receipt. Historical entries are muted and labelled.
     - exact work identities in a disclosure with copy actions.
3. **Dependencies.**
   - Rendered when any edge exists. Layout is pure: Kahn layering by depth.
   - Nodes:
     - units of this change, tone from the lane;
     - external prerequisites and dependents, labelled with their change, or with a short identity and "not current".
   - Nodes that sit on a cycle go in a final column marked as a cycle. The graph never loops.
   - SVG edges reuse `.dag-*`. A node opens its unit or change.
4. **Campaigns.** One card per membership:
   - campaign link;
   - work binding badge, stage, selection and goal;
   - admission and enumeration;
   - hold (operation, owner, reason as display text);
   - pending effects, remaining obligations and member blockers.
5. **About this view.** A disclosure with repository and view identities (copyable), snapshot facts and truncation, non-claims and the stage and lane legend.

A requested change that is missing or malformed is stated with its identity, and no other change is substituted. The generation guard and the echo check remain.

**Rationale:** the detail answers the reader's questions in order of urgency. It keeps every existing fact reachable and turns name-string joins into links.

### Decision: Inline decisions reuse the campaign answer path

**Choice:** `inventory.js` splits `decisionArticle` into its facts and a `decisionAnswer(inbox, decision, send, buttons)` builder, which the campaign view keeps using unchanged. It exports:
- `decisionAnswer`;
- `readInbox(ctx, campaignId)` (read plus validation);
- `commandRunner`;
- `runnerUi`.

For each campaign that holds this change's decisions, the Cairn page reads the inbox once. It matches inbox decisions to the projection's decisions by exact identity, restricted to this change's member keys. It mounts only the answer and issue actions the inbox lists for that decision, with one status line and one retry per campaign. After an applied command or a conflict, the page reloads the change.

The page shows the decision read-only, with a link to the campaign view, in any of these cases:
- the inbox is unavailable;
- controls are not configured;
- the principal holds no action.

Pause, continue, cancel and start are never offered here.

**Rationale:** the authority-sensitive behavior has one implementation and its tests. The campaign spec's console scenario holds on both pages, and the Cairn projection remains read-only.

## Verification Strategy

Rust, in `cairn_workspace/tests.rs`, with positive and negative cases:
- lane precedence for each stop, for `not_admitted`, for each advance, for historical-only evidence and for an unrecognized execution value;
- change lanes for empty, mixed and stopped changes;
- lane counts that sum to units;
- open decisions;
- blocker rank, subject kind and campaign identity;
- dependents, including a dependent in another change.

The explorer example test asserts the synthetic A, B, C and D lanes.

Node, in `tests/live-console-ui.test.mjs` and `tests/inventory-ui.test.mjs`:
- `v2` admission and `v1` refusal;
- lane and unknown-lane words;
- deterministic register order;
- text, lane, blocker and decision filters with counts;
- neighbors in filtered order;
- unit route normalization;
- graph layering with external, not-current and cyclic nodes;
- late-response and clearing cases;
- inline decisions offered only for listed actions and this change's members, with the store version, request-ID reuse and conflict reload carried by the shared runner.

Run once afterward:
- `cargo test --locked -p kiln-web-service`;
- crate `rustfmt` and Clippy;
- `node --check` on the touched modules;
- the Node console rail.

Browser review uses real working-tree assets in the exploration workspace:
- register list and board, filters and URL restoration;
- keyboard movement;
- a change's next steps, an inline answer, unit expansion and its timeline, the A→B graph and campaign links;
- 1440, 768 and 390 px in light and dark;
- overflow and console errors.

The review is recorded as bounded evidence. Cairn validation and traceability run with this change's delta root registered.

## Risks / Trade-offs

- **Two independent reads.** The inbox and the Cairn snapshot can disagree. A stale answer gets the owner's conflict and reloads both reads; nothing is applied from the snapshot.
- **Lanes are a summary.** The lane rule is fixed in the projection and documented in the legend. Facets stay visible in the matrix and in the cards' lane bars.
- **Detail replaces the register.** As with Workers, orientation comes from All changes, previous and next, Browse changes and the URL.
- **Index size.** The index stays bounded by `MAX_INDEX_CHANGES` and the existing 4 MiB response bound. The added fields are counts and one blocker.
