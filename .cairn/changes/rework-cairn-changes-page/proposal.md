## Why

The operator asked for the Cairn changes page (2.7) to be reworked so it is interactive and makes sense. The page joins the right facts, but it does not show what they mean:

- Every register row in the exploration workspace reads "1 blocker · 1 campaign". The blocker-count sort becomes alphabetical, and no row shows where its change stands.
- With no change selected, the main panel shows a 27-badge glossary rather than state.
- A selected change is about 2,600 px of definition lists headed by full 64-character identities. Its five stage facets read as five equal badges, which hides the progression from ready through executed, verified and integrated to recorded.
- Blockers, units, campaign members and decisions are separate lists that share only name strings. Nothing links a blocker to its subject, to the place where it is resolved, or to a run. The decisions section says to answer in the campaign's own view but gives no link.
- The page has no keyboard movement, no filter beyond text, and no URL-bound unit.

The operator reviewed alternative patterns and chose to adopt all of these:
- an attention-first register with stage progress (Linear display options, OpenSpec UI repository tiles, Zuul queue progress);
- a board of changes (OpenSpec UI, `cairn view kanban`);
- a blocking checklist that names the owner and the resolving action (Gerrit submit requirements, the GitHub merge box);
- a unit-by-stage matrix (Argo CD's separate sync and health, Zuul and GitLab job grids);
- an attempt timeline (Temporal event history);
- a dependency graph (Zuul dependency dots, `cairn change graph`, the run DAG);
- inline answers to the change's own campaign decisions.

## What Changes

- **Projection (`kiln.cairn-workspace-index.v2`, `kiln.cairn-workspace-detail.v2`):**
  - Each unit gets a board lane derived from its five facets. A stop goes to Needs attention. A unit never advances on historical evidence, and an unrecognized value never advances.
  - Each change gets a lane: Needs attention if any unit is stopped, otherwise the lane of its least-advanced unit, or Awaiting work when no unit is named.
  - The index and the overview carry lane counts and the number of open decisions.
  - The index carries the first blocker. Blockers sort by a fixed condition rank and record whether the subject is a unit or a campaign member, and which campaign.
  - Each current work version lists the current work that depends on it.
- **Register:** a full-width list and a board over the same rows, filtered at once by text, lane, blockers and open decisions, with counts. Each row and card shows the lane, how the change's units are distributed across lanes, the first blocker in words and the number of open decisions. The arrow keys move between visible rows and cards. View and filters restore from the URL, which is not re-read on every keystroke.
- **Change detail**, Workers convention: All changes, previous and next change, and Browse changes with the filters intact. The order is:
  1. Next steps. Each blocker gives its condition, subject link, owner, operation, display-only detail and campaign link. Open decisions follow.
  2. A unit-by-stage matrix. Expanding a unit is bound to the URL and shows its acceptance obligations, dependencies and dependents, superseded versions and an attempt timeline that keeps current and historical evidence apart.
  3. A dependency graph.
  4. Campaign members.
  5. A disclosure with the exact identities, snapshot bounds, non-claims and the stage legend.
- **Inline decisions:** for this change's own members, the page shows the campaign inbox's answer and issue actions. It reuses the campaign view's answer form and command runner unchanged: the same command route, store-version expectation, request-ID replay and conflict reload. Pause, continue, cancel and start stay on the campaign view. Viewers and services without campaign controls see the decision read-only, with a campaign link.
- Modify `kiln.cairn.workspace`: the projection and routes stay read-only; the page may offer only the admitted decision commands.

## Impact

- **Consumer:** operators and viewers who follow Cairn changes through canonical work, attempts and campaigns.
- **Immediate outcome:**
  - see which changes need attention and where each change stands;
  - see why a change is stuck, who owns the next step, and where to act;
  - answer the change's decisions where the blocker is shown.
- **Durable capability:** projection-owned lanes shared by the index and detail, one Cairn page module with pure presentation helpers (ordering, filtering, neighbors, graph layering), and the campaign answer path reused rather than copied.
- **Owner:** Kiln web-service and dashboard maintainers.
- **Files:**
  - `crates/kiln-web-service/src/{cairn_workspace.rs,cairn_workspace/tests.rs,cairn.js,cairn.css,inventory.js,web.js}`
  - `crates/kiln-web-service/examples/explore/campaign.rs`
  - `tests/{live-console-ui.test.mjs,inventory-ui.test.mjs}`
  - `docs/{operator.md,console-port-inventory.json}`
  - `CHANGELOG.md`, `cairn-policy/development-coverage.ncl` and this package
- **Verification:**
  - positive and negative Rust projection cases and Node presentation cases;
  - crate Cargo tests, rustfmt and Clippy;
  - the Node console rail;
  - a browser review over the real exploration stores at desktop, tablet and phone widths in light and dark;
  - Cairn gates, validation and traceability.
- **Authority:** working-copy implementation and verification only. There is no deployment, live campaign command, commit, publication or archive.

## Out of Scope

- Pause, continue, cancel or start from this page.
- Any new campaign command or owner operation.
- Reconciliation or replanning actions.
- Capability-restricted observers (#69/#70/#75/#76).
- Typed Cairn requirement references (#83).
- The Rats/Meteorite port (#67/#68).
- Live updates or polling.
- A new font, icon or remote dependency.
