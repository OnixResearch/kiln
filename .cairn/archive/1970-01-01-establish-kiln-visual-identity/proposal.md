## Why

The operator asked for a UI and UX that is beautiful and unique. Kiln's four browser surfaces carry four unrelated looks today:

- the live console follows another product's console reference (off-white, charcoal, acid yellow), adopted by `align-blacksmith-visual-design`;
- the offline catalog is olive and terracotta with a serif letter mark;
- the offline report has a dark rail with an orange accent;
- the hosted bridge page is a separate dark theme with hard-coded colours.

Each surface defines its own palette under its own token names, and the brand mark is a different letter in a different box on each. Nothing in the presentation is recognisably Kiln, a palette change must be made in four places, and dark values are written twice per stylesheet. The live console also gives no signal outside the Overview and Runs pages that work is running, and no feedback while a slow read is in flight.

## What Changes

- Add one shared identity stylesheet, `crates/kiln-dashboard/src/render/identity.css`. It holds the palette for light and dark, type stacks, scale, radii, elevation, motion and the brand mark, under the existing GPUI Kit–aligned semantic token names. Every surface includes it and no surface keeps its own palette.
- Give Kiln its own identity: warm porcelain and soot neutrals; ember for action emphasis and focus; glaze-derived status colours (copper green, oxblood, cobalt, ash yellow); a kiln-arch brand mark drawn in CSS; a graduated "pyrometer" rule under page headings; a dark soot navigation rail in both appearances.
- Live console: restyle the shell and every shared component on the identity, including sign-in, empty, loading and unavailable states, the run inspector and the log panel. Routes, labels, controls and confirmations are unchanged.
- Live console UX:
  - a running-run count in the top bar on every page, taken from the last admitted live snapshot and linking to the pending-run list; the brand mark glows while that count is above zero, and both are hidden while observation is stale;
  - a non-blocking progress line while a read or mutation is in flight (background observation polls excluded);
  - an appearance switch in the account menu, beside the existing Settings page;
  - a Kiln favicon served from Kiln's own origin;
  - short entrance transitions for menus and dialogs only, removed under reduced motion; page changes stay immediate.
- Offline catalog and report: the same identity and a new overview hero. The generated index widget is unchanged; the catalog stylesheet restates the three inline cell colours it writes with identity tokens. The report's outcome column holds the longest outcome name on one line.
- Hosted bridge page: the same identity in light and dark, replacing its hard-coded dark theme. A table of record rows wider than its panel scrolls inside a labelled, focusable region.
- Offline catalog, offline report and hosted bridge page: at 960px and narrower, buttons, links, fields, selects and disclosure headings are at least 44px tall, as the console's already are.
- Live console tables (Overview project list, run list, a project's run history, code-review change list): rows stack when the table's own region is 48rem or narrower, so a project name or change title is no longer squeezed into broken words and a status label is not cut.
- Repair two rendering defects found during the browser review: the code-review change list went blank for a principal with saved personal views, and every status lane heading of a work board read `[object HTMLSpanElement]`.
- Modify accepted requirement `kiln.web.visual_design`: the console presents the Kiln identity with ember action emphasis instead of acid yellow, and its run, project and change lists stack in a narrow region with whole status labels. Add the `visual-identity` capability for the shared layer, its contrast floor, its self-containment and the narrow-screen control size, and add `kiln.web.live_activity` and `kiln.web.request_feedback` to `live-console-design`.

## Impact

- **Current consumer:** the operator and Kiln developers who use the live console daily, and readers of offline catalog and report files.
- **Immediate outcome:** every Kiln browser surface shows one recognisable identity, and the console reports running work and in-flight requests on every page.
- **Durable capability:** one identity layer. A palette, type or mark change is made once and reaches the console, the catalog, the report and the hosted page. A deterministic check holds the contrast floor and rejects undefined tokens and palette literals outside the identity.
- **Owner:** Kiln web-service and dashboard maintainers.
- **Files:** `crates/kiln-dashboard/src/render/{identity.css,catalog.css,html.css,html.rs}`, `crates/kiln-dashboard/src/catalog.rs`, `crates/kiln-web-service/src/{web.html,web.css,web.js,ui.js,source.css,code-review.css,project.css,server.rs,favicon.svg,code-review.js,work.js}`, `crates/kiln-hosted/src/{web.html,web.css,web.js,bridge.rs}`, `tests/kiln-identity.test.mjs`, `tests/live-console-ui.test.mjs`, `flake.nix` (check command), `cairn-policy/development-coverage.ncl` and `cairn-policy/PROVENANCE.md` (delta-spec roots of this change while it is in flight), `docs/operator.md`, `docs/observation-dashboard.md`, `docs/hosted-ci.md`, `CHANGELOG.md` and this package. `work.css` and `work-interaction.css` already use tokens only and take the identity without an edit; `code-review.css` and `project.css` change only where their tables stack.
- **Testing:** node tests for the contrast floor, token definition, palette ownership, the running-count rule and its stale-observation rule, each with a negative case; `cargo test` and Clippy for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`; a browser review of the real console assets over synthetic data and of real generated catalog and report files at 320, 768, 1440 and 1920px in light and dark, covering overflow, touch targets, focus, reduced motion, console errors and off-origin requests, and a scan of the console tables across 63 widths for broken words and cut status labels.
- **Authority:** presentation only. Lifecycle acceptance, synchronisation into accepted specs, deployment and publication stay with the operator.

## Out of Scope

- No API, route, storage, identity, authority, execution or provider change. Canonical outcome names, human labels and view vocabulary are unchanged.
- No bundled web font, no remote asset and no icon outside the pinned Lucide set. The brand mark is Kiln artwork, not an icon.
- The terminal report and CLI output are unchanged.
- No deployment, publication, spec synchronisation or archive. A browser review over synthetic data is not evidence of real CI execution or of deployed bytes.
