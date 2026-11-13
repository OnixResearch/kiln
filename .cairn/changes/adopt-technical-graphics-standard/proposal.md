## Why

The operator asked for every page of the Kiln web surfaces to make sense to a new and to an advanced user, held to the NASA graphics standard in the manner of the U.S. Graphics Company. The identity accepted in `establish-kiln-visual-identity` gives Kiln one look, but that look is decorative: warm porcelain, ember gradients, a glowing arch, rounded panels and soft shadows. It does not read as a technical document, and it leaves the console hard to learn:

- nothing tells a new operator where things are or what a page is for beyond one sentence, and no page explains run states or Kiln's vocabulary;
- a destination's name and purpose are written in three places (the rail markup, a title map and a description map in `web.js`), and the command palette offers only eight of the twenty destinations;
- the brand returns to Work, not to the Overview that a bare URL opens;
- a running run's duration and a queued run's start read "Not recorded", as if data were missing;
- retained usage reads `1288490188 / 1610612736` with a snake-case resource name;
- a team workflow status renders as an unknown state;
- CI projects puts two forms above the inventory most readers came for.

## What Changes

- Adopt a technical graphics standard in `identity.css`: neutral paper and ink, inverted in dark appearance; one identity red (NASA red) reserved for the brand mark, the title rule's index block and the request-progress line; a solid ink block for each view's single primary action; a Helvetica-class grotesque for reading text and a monospace technical-label layer (eyebrows, column heads, field labels, fact terms); square geometry (every radius tier zero); flat surfaces with hairlines and ink frames; and an unblurred offset shadow for overlays only. Status colours keep their meanings.
- Redraw the brand mark as a square field of identity red with an ink kiln door, amber while the live snapshot reports running work. The favicon follows the mark.
- Restyle every surface on the standard: the console shell, rail, top bar, title block, panels, tables, tabs, chips, forms, dialogs, the command palette, run inspector and log panel; the work, code-review, project and source pages; the offline catalog and report; the hosted bridge page.
- Add one numbered destination register to `ui.js`: section numbers in five groups (0 start here, 1 continuous integration, 2 work, 3 inspect, 4 administration), each with its title and purpose. The rail, title block, breadcrumb and command palette read it, and the console admits exactly its pages.
- Add a Guide page (0.0): a first path for a new operator, every destination by number with its purpose, how each run state reads, the words Kiln uses and the keyboard reference, including code review's shortcut table.
- Move the rail's CI group first, return the brand to the Overview, add the Guide to the rail footer and Settings, and keep the current destination scrolled into the rail's view.
- Name timing a run cannot have yet: In progress (running duration), Not started (waiting start and duration), Not finished (unfinished completion). Finished runs with absent timing still read Not recorded.
- Show retention quantities in binary units with their exact byte count and share of the limit, and name the resource in words. Move `humanBytes` from `source.js` into `ui.js` so every page uses one formatter.
- Lead CI projects with the inventory; state started, requested or finished time on project cards by run state; give empty Overview two next steps (CI projects, Guide); render a team workflow status as a neutral custom cell; give worker slot badges their signal tones; align the Guide tables, the stacked run-list sort controls and the Guide's numbered rows.
- Restore pressed feedback for primary, danger and quiet buttons in the flat grammar.
- Add `kiln.identity.graphics_standard` with a deterministic check; extend `kiln.identity.contrast` to frames, selected rows and the mark; modify `kiln.web.visual_design` and `kiln.web.readable_observations`; add `kiln.web.wayfinding`.

## Impact

- **Current consumer:** the operator and Kiln developers who use the live console daily, operators new to Kiln, and readers of offline catalog and report files and of the hosted bridge page.
- **Immediate outcome:** every Kiln surface reads as one technical document, and a new operator can find any destination by number, learn what each run state means and see what to do first.
- **Durable capability:** one destination register that the rail, title block, breadcrumb, palette and Guide cannot drift from, and a deterministic standard check that rejects identity red outside its three places, rounded geometry and blurred shadows in every surface stylesheet.
- **Owner:** Kiln web-service and dashboard maintainers.
- **Files:** `crates/kiln-dashboard/src/render/{identity.css,catalog.css,html.css}`, `crates/kiln-web-service/src/{web.html,web.css,web.js,ui.js,source.js,project.js,work.js,work-interaction.js,work.css,work-interaction.css,code-review.css,project.css,source.css,favicon.svg}`, `crates/kiln-hosted/src/web.css`, `tests/kiln-identity.test.mjs`, `tests/live-console-ui.test.mjs`, `cairn-policy/development-coverage.ncl` (this change's delta-spec roots while it is in flight), `docs/operator.md`, `docs/observation-dashboard.md`, `CHANGELOG.md` and this package.
- **Testing:** node tests for the contrast floor with the new frame, selection and mark pairs, the identity-red/failure distance, the standard check (identity red, radius, shadow) with failing fixtures, the destination register's invariants, pending run timing including finished, malformed and recorded cases, and binary byte units; `cargo test --locked` for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`, which embed the assets; a browser review of the real assets over synthetic data.
- **Authority:** presentation only. Lifecycle acceptance, synchronisation into accepted specs, deployment and publication stay with the operator.

## Out of Scope

- No API, route, storage, identity, authority, execution or provider change. Canonical outcome names, existing human labels, page URLs and confirmation dialogs are unchanged; the console admits the same pages plus `guide`.
- No bundled web font, no remote asset and no icon outside the pinned Lucide set. The NASA "worm" logotype is NASA's mark and is not used; the standard borrows its discipline, not its artwork.
- The terminal report and CLI output are unchanged.
- No deployment, publication, spec synchronisation or archive. A browser review over synthetic data is not evidence of real CI execution or of deployed bytes.
