# Console port inventory

<!-- r[related kiln.web.console_inventory] -->
The live console served by `kiln-web-service` is the reference for its port to
Rats/Meteorite ([#68](https://github.com/OnixResearch/kiln/issues/68)). It stays
the default entry point, and its CSS, assets, icon geometry and font policy stay
authoritative, until each route cohort passes its own port gate.
[`console-port-inventory.json`](console-port-inventory.json) (schema
`kiln.console-inventory.v1`) records what defines each surface today.
`tests/console-inventory.test.mjs` checks it against the served console and runs
in the `live-console-ui` Nix check.

## What a route records

| Field | Meaning | Checked against |
| --- | --- | --- |
| `page`, `number` | Destination key and section number | The register in `ui.js` (`KilnUi.destinations`); every registered destination appears exactly once |
| `cohort` | Rollout cohort | The declared cohorts; each holds at least one route, and `runs` is in the first |
| `panel` | Page container | `<section data-panel="…">` in `web.html`; top-level pages also need a rail link |
| `modules` | Renderer and supporting scripts, with the global each defines | Served by `server.rs`, loaded by `web.html` or by the named loader module, `window.<global> =` in the module, and dispatch from `web.js` for a renderer global |
| `node_tests` | Node tests that load the module | The test file exists and loads that module |
| `stylesheets` | Page stylesheets beyond the shared ones | Served by `server.rs` and linked by `web.html` |
| `api` | API paths the page's modules request | Named as a path string by one of its modules |
| `url_state` | Route parameters parsed for the page's renderer | `params.get("…")` in `web.js` `readRoute` |
| `states` | States the port's parity cases must exercise where fixtures support them | The inventory's state vocabulary |

Shell components (brand mark, navigation rail, top bar, command palette, page
header, notice, confirmation dialog and sign-in) record their DOM anchors, owner
module and stylesheets; classes the owner creates at run time must appear in the
owner and in a listed stylesheet. `identity.css`, `catalog.css` and `web.css` are
shared by every route.

A failing check names the entry and the missing reference, for example
`route source: module source-tree.js does not exist`. Update the inventory in the
same change that adds, removes or renames a destination, panel, module, global,
stylesheet, anchor, API path or Node test.

For **Delivery & Cairn** (2.1), inventory both read-only workspace GET routes and
the shared campaign inbox/command paths used solely for inbox-admitted answer
and issue actions. Its URL state records repository, change, expanded unit,
search, lane, blocker and open-decision filters, and List/Board view. The
inventory is a description of the served console, not a new command surface:
the canonical campaign owner still admits every decision command separately.

For **Fleet & Workers** (3.1), live updates arrive over the shared
multiplexed `/api/events` SSE stream (`fleet` frames) backed by a single
coordinator watcher in `kiln-web-service`. The `/api/fleet/*` paths carry the
machine slot census, visible-only live `herdr` pane reads, dispatch pause/resume,
and operator claim/attempt/decision commands. The fleet service remains the
authority for every command.

## Cohorts

| Order | Cohort | Routes |
| --- | --- | --- |
| 1 | Runs pilot and shared shell | 0.0 Guide, 1.1 Factory, 2.2 Runs & Evidence |
| 2 | Work and planning | 1.2 Attention (incl. Requests & triage), 2.1 Delivery & Cairn (incl. Issues, Planning, Saved views), 3.1 Fleet & Workers |
| 3 | Code review | 2.3 Code Review & Queue |
| 4 | Projects and source | 2.2 CI projects / CI project / Runbooks, 3.2 System & Source |
| 5 | Workers and inventories | 3.1 Worker targets |
| 6 | Receipts, feeds, administration and the rest | 2.2 Patchgate receipts / Evidence, 3.2 Activity / Feeds / Automation / Operations / Raw collaboration history, 4.4 Settings |

Overview and the Guide join the first cohort because they are the shell's landing
and orientation surfaces. The order follows #68's rollout list; it does not
qualify any route.

## Recorded coverage

Every route's renderer is covered by Node module tests in addition to Rust route
tests and live console rails:
- `tests/live-console-ui.test.mjs` loads `ui.js`, `source.js`, `receipts.js`, `workers.js`, `runbooks.js`, `cairn.js`, and `code-review.js`.
- `tests/inventory-ui.test.mjs` loads `inventory.js`.
- `tests/preview-compare.test.mjs` loads `preview-review.js`.
- `tests/work-views.test.mjs` loads `work-board.js`.
- `tests/web-module.test.mjs` loads `web.js` (Guide, Overview, Runs, CI projects, Activity, Feeds, Evidence, Automation, Operations, Raw collaboration history, Settings).
- `tests/fleet-ui.test.mjs` loads `fleet.js` (Fleet & Workers coordinator, machine census, claims/attempt controls, Fair-Admission & SSE handling).
- `tests/work-module.test.mjs` loads `work.js` and `work-evidence.js` (Issues, Inbox, Planning, Triage, Saved views).
- `tests/project-ui.test.mjs` loads `project.js` (CI project).

Browser-level rails (the service smoke, the Bombadil exploration and the
explorer's named examples) complement these Node module tests for full end-to-end
visual parity through #66 before porting a cohort.

## Preservation rules

- Keep the existing CSS and assets authoritative during the first port; token or
  schema cleanup is separate work.
- Action emphasis uses `--primary` (ink and paper), never identity red. Failure
  keeps its own danger color and unknown stays visibly distinct. The brand door
  is lit only from the current admitted running-work observation.
- Keep the exact font stacks and SVG paths, viewboxes and strokes. A different
  default font, a nearest-match icon, a wrapper layout or a stock component
  treatment is a visual change and cannot become the new baseline silently.
- Keep real navigation anchors, copy and open-in-new-tab, history and deep links,
  form validation, keyboard focus, dialog dismissal and focus restoration,
  selection and scroll position, and the loading, disconnected, stale and unknown
  states. Logs keep their explicit gap, truncation, stream and cursor
  distinctions; unavailable reports and artifacts stay unavailable.
- Kiln keeps route and object identities, accepted API observations, outcome
  meaning and command validation. Shared components receive view data and emit
  intents; no mirrored CI model moves into frontend state.

## Non-claims

The inventory describes structure and existing checks. It is not visual parity
evidence, does not port or qualify a route, and grants no CI, review or execution
authority. The Meteorite bundle and renderer lifecycle (#67) and capture parity
(#66) remain prerequisites for every cohort.
