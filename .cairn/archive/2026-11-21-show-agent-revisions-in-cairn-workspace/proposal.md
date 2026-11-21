## Why

Kiln #84's Cairn workspace joins canonical work, attempts, campaigns and
lifecycle receipts for one Cairn change, but it shows nothing about the agents
that would run that work. Two Lattice criteria name this workspace as their
consumer:

- Lattice #24: "Publish redacted bounded capability and wait reasons to Kiln
  #60 and Cairn workspace #84, preserving hidden names under existing
  disclosure rules."
- Lattice #25: "Expose selected/pinned workflow and worker revisions
  separately in Kiln workspace #84, without leaking restricted context."

Kiln #60 already reads `lattice.worker-inventory.v3` into the provider-neutral
`kiln_core::inventory` model (change `consume-lattice-inventory-v3`): each
definition's desired, prepared, qualified and selected revision with its
`unmanaged`, `unknown` or `recorded` lifecycle state, task-family
qualification references with evidence class and `fixture` or `live`
provenance, each worker's revision pin and blocked state, and owner-attributed
wait reasons. The workspace does not read that model, so an operator looking
at a Cairn change cannot see which revision new sessions select, which
revision each worker is pinned to, or which owner a wait is on.

## What Changes

- The Cairn change detail gains an `agents` section, computed by a pure
  projection from Kiln's admitted inventory v2 projection, never from producer
  documents:
  - per definition: its observation state, profile revision, revision
    management, the four revision slots (`desired`, `prepared`, `qualified`,
    `selected_for_new_sessions`) each as `recorded`, `not_recorded`, `unknown`
    with its owner, `unmanaged` or `unpublished`, the rollout generation of a
    recorded selection, declared capability (task families, tools, harnesses,
    output contracts, models) as `declared`, `none` or `unknown`, task-family
    qualification references with evidence class and provenance, and its wait
    reasons;
  - per worker: its observation state, lifecycle, pinned revision, whether it
    matches the definition's selected revision, its pin (`recorded` with
    qualification, rollout generation, identity, status and `blocked`,
    `unmanaged`, `unknown` with its owner, or `unpublished`), whether its
    attempt is one of this change's attempts, and its wait reasons;
  - workflow revisions as a separate slot that Lattice does not publish yet:
    `unpublished`, owner `lattice#25`.
- A `fixture` qualification reference never counts as live evidence. Sessions,
  workspaces, execution allocations and grants, model-service bindings, full
  qualification records, preparation failures and the attempts of other work
  are never copied into the section.
- The detail route reads the configured `--inventory` input through the
  existing `read_inventory` shell, with the principal and clock the inventory
  route uses. An unconfigured or unavailable inventory is reported as such with
  its refusal code; it never fails the Cairn detail or reads as "no agents".
- The detail schema becomes `kiln.cairn-workspace-detail.v3`. The index is
  unchanged.
- The Cairn changes page renders an **Agents and revisions** section with
  distinct words and tones for each slot state, provenance and pin, and the
  workflow-revision slot apart from worker revisions.

## Impact

- **Consumer:** operators reading a Cairn change; Lattice #24 and #25 name the
  workspace as the consumer of these facts.
- **Files:** `crates/kiln-web-service/src/cairn_workspace.rs`,
  `cairn_workspace/{agents.rs,agents_tests.rs,http.rs,tests.rs}`,
  `server.rs`, `cairn.js`, `cairn.css`,
  `crates/kiln-web-service/examples/{explore.rs,explore/campaign.rs}`,
  `tests/live-console-ui.test.mjs`, `docs/operator.md`,
  `docs/inventory-producers.md`, `CHANGELOG.md` and this change package.
- **Verification:** projection tests over the Lattice v3 producer fixtures
  (unmanaged, unknown with owner, recorded, revoked, fixture and live
  provenance, redaction, source states, change attempts), detail and route
  tests, Node UI tests, and the repository gates.
- **Authority:** read-only. No reservation, dispatch, selection, promotion or
  provider contact.

## Out of Scope

The rest of #84 stays open: typed Cairn requirement and acceptance-criterion
references and native obligation explanations (#83, Cairn #14/#12),
capability-restricted observers (#69/#70/#75/#76/#79), the retained A→B, C, D
example with a restricted observer, and the renderer migration (#67/#68).
Lattice does not publish workflow revisions or per-definition capability
restrictions yet; Kiln does not invent them. Readiness explanation for a unit's
exact work stays on the inventory route (#60).
