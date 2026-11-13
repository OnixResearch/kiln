## Context

Kiln renders four browser surfaces from three crates:

| Surface | Stylesheets | Palette today |
| --- | --- | --- |
| Live console (`kiln-web-service`) | `catalog.css` (shared shell), then `web.css`, `work.css`, `work-interaction.css`, `code-review.css`, `project.css`, `source.css` | off-white, charcoal, acid yellow; GPUI Kit–aligned token names; light, a media-query dark block and a repeated `data-theme="dark"` block |
| Offline catalog (`kiln-dashboard`) | `catalog.css`, inline | olive and terracotta under its own token names |
| Offline report (`kiln-dashboard`) | `html.css`, inline | dark rail, orange accent, a third set of names |
| Hosted bridge page (`kiln-hosted`) | `web.css` | dark only, literal colours |

`web.css` carries a block that maps the catalog's token names onto its own and restates rules because the catalog's `--muted` is a text colour while GPUI Kit's `muted` is a background. The console's page stylesheets already use tokens only: they contain no colour literal.

Accepted requirements bind this work: `kiln.web.visual_design` (modified here), `kiln.web.design_system` (GPUI Kit token vocabulary, pinned Lucide icons, Design Guides), `kiln.web.minimal_console`, `kiln.web.console_layout`, `kiln.web.page_consistency`, `kiln.dashboard.product.*` and `kiln.observe.dashboard.html.*`. The console and hosted page are served under `Content-Security-Policy: default-src 'none'; style-src 'self'; …`, so fonts cannot load and the hosted page cannot load images.

## Goals and Success Contract

One recognisable Kiln identity on every surface, declared once. The console states running work and in-flight requests on every page. Nothing an operator can do today stops working.

False completion includes: a surface that still declares its own palette; dark values that exist only for the console; a mark that differs between surfaces; a running count that survives a stale connection; a progress indication that stays after a failed request; an animation that remains under reduced motion; a contrast claim that no check holds; a changed label, route, confirmation or keyboard path.

## Decisions

### Decision: One identity stylesheet, included by every surface

**Choice:** Add `crates/kiln-dashboard/src/render/identity.css`. It defines every colour token, the type stacks, the spacing, type, radius, elevation and motion scales, the brand mark (`.brand-mark`) and the heading rule (`.kiln-rule`). Inclusion is static:

- report: `html.rs` writes `identity.css` then `html.css` into the one `<style>`;
- catalog: `catalog.rs` writes `identity.css` then `catalog.css`;
- console: `server.rs` serves `/identity.css`; `web.html` links it first;
- hosted page: `bridge.rs` serves `/app.css` as `identity.css` followed by its `web.css`, joined at compile time.

`catalog.css`, `html.css` and the hosted `web.css` move to the semantic token names. The mapping block and the restated rules in `web.css` are deleted.

**Rationale:** One file is the only place a palette, type or mark change is made. The console already reads `catalog.css` from the dashboard crate by path, so the identity uses the same established include and adds no Cargo dependency.

**Rejected:** a copy of the palette per stylesheet (four copies drift, which is the present state); a new shared crate for one CSS file; serving the hosted page a second stylesheet route (more markup and policy surface than a compile-time join).

### Decision: Each colour declared once with `light-dark()`

**Choice:** `:root` sets `color-scheme: light dark`; `data-theme="light"` and `data-theme="dark"` set `color-scheme` to one value. Every colour token is `light-dark(<light>, <dark>)`. A surface that is dark in both appearances (navigation rail, log panel, catalog hero) sets `color-scheme: dark` and declares no colours.

A dark region that can itself take focus (the console's log output, the hosted response panel, the catalog's index table) draws its focus ring inside its own edge. The ring takes the region's dark value; drawn outside, it sat on the light page at 2.06:1 on the hosted panel and on the index table. Inside, it sits on the region's own surface.

**Rationale:** The three theme blocks per stylesheet collapse into one, so light and dark cannot drift. The value resolves where a token is used, so a dark region inside a light page needs one declaration instead of a second token table; the log panel's 25-token override is removed.

**Rejected:** keeping duplicated dark blocks (the defect this removes); a build step to generate them (no bundler exists and none is added).

**Bound:** `light-dark()` needs Chromium 123, Firefox 120 or Safari 17.5. The stylesheets already require `:has()`, container queries and `color-mix()`.

### Decision: Clay, soot, ember and glaze

**Choice:**

- Neutrals are warm: porcelain `#f4f1eb` and soot `#131110` backgrounds, with surface, border and text steps in the same hue.
- Ember `#ff6a2b` is `primary`: the single primary action, the active navigation tile and the live-activity signal. Its text is `#2b1004`. `ring` is a darker ember in light appearance so focus reaches 3:1.
- Status colours come from glazes and keep their meanings: copper green (success), oxblood (failure), cobalt (running, information), ash yellow (warning), bisque with a dashed boundary (unknown), slip greys (queued, cancelled).
- `link` is ink, the foreground colour. Links are underlined, with an ember underline. In-app destinations keep their row and button styles.

**Rationale:** The identity derives from the product's name instead of another product's console. Ember as action emphasis stays apart from every status hue: failure is a cool, dark red, warning is yellow, running is blue. Running stays blue rather than ember because red, orange and green segments side by side are hard to tell apart with colour-vision deficiency. Ink links avoid the present collision where `link` and `success` are nearly the same green.

**Rejected:** keeping acid yellow (the look is another product's signature, which is what "unique" excludes); ember for the running state (collides with failure in adjacent bar segments); a second accent hue for links (spends the emphasis budget the Design Guides ask to keep scarce).

### Decision: Brand mark is CSS artwork; the favicon is one served file

**Choice:** `.brand-mark` is an ember kiln arch with a dark firing door, drawn with gradients, sized in `em`, with no letter. `kiln-web-service` serves the same shape at `/favicon.svg`. The offline documents and the hosted page add no favicon.

**Rationale:** The mark must render under `style-src 'self'` with no image request and inside self-contained HTML. It is Kiln artwork, not an interface icon, so `kiln.web.design_system` (every icon from pinned Lucide) is unaffected and `web.html` still contains no `<svg`. The hosted policy has no `img-src`; widening a security header for a favicon is not justified.

### Decision: Dark navigation rail in both appearances

**Choice:** The console rail, the catalog sidebar and the report sidebar set `color-scheme: dark`. The active destination keeps its own selected fill; its icon tile is ember.

**Rationale:** The rail is the stable frame the Design Guides ask for, and one dark band with the lit mark is the identity's anchor on every surface. Selection stays on the item's own surface, with no leading-edge bar.

### Decision: Flat surfaces, a graduated heading rule, elevation only for overlays

**Choice:** Panels and cards are a fill and a hairline with no shadow. Menus, dialogs, the command palette and the expanded rail keep shadows. The page heading's bottom boundary is a graduated tick rule that fades out (`.kiln-rule`); the pass-rate bar carries the same ticks. A faint ember wash sits behind the top of the page.

**Rationale:** The Design Guides reserve elevation for stacking. The tick rule replaces an existing line, adds no area and gives the measurement character that fits a tool whose claims are bounded observations.

### Decision: Running count and its presentation are pure functions; the shell only draws

**Choice:** `KilnUi.liveActivity(runs)` returns `{ running }`: the number of entries whose `outcomeStatus(outcome, phase).status` is `running`. A value that is not an array yields zero and entries that are not objects are skipped. `web.js` stores the result when a live snapshot is admitted. `KilnUi.presentedRunning(running, current)` returns the count to show: the stored count when `current` is exactly `true` and the count is a positive safe integer, otherwise zero. `connection(message, healthy)`, the one place that already sets the live indicator, passes `healthy` as `current` and draws the result: a top-bar link (`N running`, to the retained-run list filtered to `Pending`, where each row states running, queued or awaiting approval) and `data-firing` on the document element, both only while the result is above zero. Stale observation and `logout` therefore hide both; `logout` and Refresh also discard the stored count.

The service's live endpoint answers with a heartbeat instead of a snapshot when its durable state version has not moved past the version the console sent. A heartbeat after a recovered connection therefore confirms that the stored count is still the current one, and the link returns without a new snapshot.

**Rationale:** The count reuses the one rule that already labels a run Running, so the signal cannot disagree with a status pill. Both rules are tested without a browser, including the stale case; the shell holds no decision.

**Failure behaviour:** an invalid snapshot already raises before adoption, so no count is derived from it. A stale or stopped stream removes the count instead of freezing it.

### Decision: Request feedback from one counter at the fetch boundary

**Choice:** `api()` is the console's only `fetch`. It takes a `quiet` option, true by default for the live-observation poll and passed by log following, the two background polls. Every other request increments a counter before the fetch and decrements it in `finally`. A non-zero counter sets `data-busy` on the document element and `aria-busy` on `#main`. CSS shows a moving ember line on the top bar's lower edge after a 150 ms delay and holds it static under reduced motion.

**Rationale:** `finally` covers success, failure, timeout and abort, so the indication cannot outlive its requests. The delay keeps immediate reads from flashing a loader, which the Design Guides call ceremony.

### Decision: Motion only where it states a real change

**Choice:** Transitions exist for overlays appearing (menus, dialogs, palette), the lit mark and running indicators, loading skeletons and the busy line. Page changes are immediate. `prefers-reduced-motion` sets motion tokens to zero and removes every animation.

**Rejected:** page entrance animation and View Transitions (the Design Guides: motion is not decoration for an immediate action).

### Decision: Index widget colours restated in the catalog stylesheet

**Choice:** The generated Rats index writes three inline cell colours and one embedded rule. `catalog.css` restates them with identity tokens through attribute selectors, and comments the coupling. The generated assets and `web/dashboard-index/` are unchanged.

**Rationale:** Changing the widget's colours at source requires regenerating the locked WebAssembly assets in the pinned Nix shell. The one Nix store operation attempted for this change, `nix flake prefetch` of the pinned GPUI Kit source, printed store deletions and did not finish in 120 s, so the regeneration is not run here. Source and generated assets must not be left disagreeing, so the widget source is left alone.

**Bound:** if the widget changes how it serialises cell styles, the selectors stop matching and the table shows its own colours again. Nothing else depends on them.

### Decision: Repair two rendering defects found during review

**Choice:**

- `code-review.js` appends to an undeclared `li` when a principal has saved personal views. The `ReferenceError` blanks the change list. Declare the list item.
- `work.js` passes the status element of a board lane as the text of a new `span`, so every status lane heading reads `[object HTMLSpanElement]`. Append the element itself.

**Rationale:** Both were found while building data for the browser check and both make a page unusable or unreadable. A page that fails to render is a UX defect inside this change's surface; each fix is one line.

### Decision: One touch layout at 960px and narrower on the offline documents and the hosted page

**Choice:** The browser review found controls under 44px at 320px and 768px in the catalog (36px icon buttons, 29px to 38px buttons, a 21px search field, 40px selects, 15px navigation links), the report (24px run and attempt buttons, 21px disclosure headings, 40px fields) and the hosted page (a 36px brand link). `catalog.css` and `html.css` add one `max-width: 60rem` block each, and the hosted `web.css` moves its control heights to the same boundary, so buttons, links, fields, selects and disclosure headings are at least 44px tall, with JavaScript and without it. The phone top bar of the catalog narrows its search field so the 44px icon buttons fit at 320px. `kiln.identity.touch_targets` states the rule.

**Rationale:** The console already keeps 44px controls below 60rem under `kiln.web.page_consistency`; a phone reader of an offline report had smaller targets than a console user. One boundary on every surface is easier to hold than three.

**Bound:** the rows of the pinned index widget keep the widget's own height. The project cards and the HTML navigation reach every history through 44px controls. An embedded report takes the rule from the width of its frame, not of the window.

**Unchanged:** line permalinks in the console's code-review diff are 32px between 481px and 960px and 44px at 480px and narrower. That is an existing `code-review.css` rule, which this change leaves as it is.

### Decision: Console tables stack by the width of their own region

**Choice:** The browser review found four defects in the console's tables, all present before this change:

- at 320px the Overview project list gave the project name a 67px column, so `Telemetry` broke inside the word;
- at 320px the run list cut the status label `Awaiting approval` to `Await…`, because the status shared its line with the start time;
- between 641px and 767px the Overview list broke `Logs` and `Pipeline` inside the word;
- at 768px the code-review change list gave the title a 125px column, and its tallest row was 444px (289px once stacked).

A table that can stack (`.data-table-stack`: the Overview project list, a project's run history, the code-review change list) now does so by a container query on its own wrapper at 48rem. The run browser's existing container query moves from 44rem to the same 48rem. In a region of 24rem or less the Overview list and a project's run history read one fact per line, and the run list gives the status and the start time a line each. `Logs` and `Pipeline` never wrap. Name and title links keep `overflow-wrap: anywhere`.

**Rationale:** The viewport is the wrong measure: the code-review list sits beside a 24rem column on a 1200px screen and is as narrow there as on a tablet. A container query asks the question that matters, which is how wide the table's region is. One threshold for both kinds of table keeps the rule statable: 48rem or less stacks. The modified `kiln.web.visual_design` states the rule and gains the scenario `kiln.web.visual_design.scenario.lists`. The layouts also serve accepted `kiln.web.page_consistency` (no meaning-hiding truncation at 320, 768, 1440 and 1920px) and `kiln.web.runs_table`, which do not change.

**Rejected:** `overflow-wrap: break-word` on names, so that a table column reserves its longest word. A name with no break opportunity then widens the table past its wrapper, and visually hidden text inside the scrolled cells widens the document: 56px and 114px of document-level overflow at 1024px with a 66-character name. `anywhere` cannot overflow, so it stays, and the threshold keeps the name column wide enough for ordinary words.

**Bound:** a single word wider than its line still breaks. With the review data the narrowest name column in table mode is 141px (run list), 179px (change list) and 200px (Overview list). In a project's run history at 320px, before the one-fact-per-line rule, an unbroken revision ran 26px past its half-width cell into the duration beside it; the review now also checks that no text leaves its cell.

### Decision: Hosted record rows scroll inside a labelled region

**Choice:** The restyled hosted page let the record table shrink to its panel, which broke values inside the word: `synced` and `pending` at 320px, `member` and `unknown` at 768px. Cells now use `overflow-wrap: break-word`, so a column reserves its longest word and the table scrolls inside `#records` when it is wider than the panel. `web.js` makes `#records` a focusable region named `Recorded response rows` while it holds a table, and removes the role, the name and the tab stop when it is empty or the request is rejected.

**Rationale:** The rows are provider records of up to seven columns; at 320px they cannot fit, and a scrolled table with whole values is readable where broken values are not. A keyboard user must be able to scroll the region, and an empty region must not be a tab stop. The page has no visually hidden text inside the table, so the scroll stays inside the region: no document-level overflow in 80 loads.

**Bound:** a peer identity is one 64-character token; it wraps inside its 20rem cell.

### Decision: The report's outcome column fits the longest outcome name

**Choice:** The outcome column of the run table is 9.25rem wide instead of 25% of the table. At the table's 29rem minimum the old share left 97px for a 119px `DomainFailure` at the restyled type size, so the name wrapped. Run and attempt identifiers share the remaining width and wrap anywhere, as before.

## GPUI Kit Design Guides

Conformance is kept: platform UI font for interface text and monospace only for identifiers, shortcuts and aligned numbers; a finite radius scale with concentric nesting; selection on the item's own surface; one emphasis colour; elevation for stacking only; motion for real change. The recorded web deviations stand unchanged (destinations are links, no OS chrome, 44px mobile targets, the page-header command stays primary). Cursor behaviour is unchanged by this change.

## Risks / Trade-offs

- `catalog.css` styles both the catalog and the console shell. Renaming its tokens or adding a touch rule can change a console rule that `web.css` does not override. The browser review compares every console page before and after.
- The report embeds the identity in each document, so a catalog carries one copy per history (about 9 KB each). Rendered-size bounds in tests and catalogs must still hold.
- A container query makes a stacking table's wrapper a containing block for positioned content. The three tables hold links, labels and status indicators only; a menu added inside one later would be clipped by the wrapper.
- Orange is close to warning yellow and failure red in hue order. The contrast check holds lightness; distinctness is held by fill (only the primary action is a solid ember fill), icon and label, and is reviewed by eye in both appearances.
- A dark rail under a light page changes the hover-expanded overlay from light to dark. Focus, labels and Escape behaviour are unchanged and re-checked.
- The synthetic data used for the browser review is shaped from the service's serializers and the console's own validation; it is not the service. Deployed behaviour is not claimed.

## Verification Strategy

Pure checks, in `node --test`:

- contrast: each text, status, action, focus and boundary pair of `identity.css` in both appearances, and a lowered colour is reported with pair, appearance and ratio;
- ownership: no colour literal in any surface stylesheet; every `var(--token)` used is defined by the identity or the same stylesheet; a fixture with a literal and one with an undefined token are both reported;
- `liveActivity`: counts running only; queued, awaiting approval, unknown, finished, malformed and non-array inputs;
- `presentedRunning`: the stored count while observation is current; none while it is stale, before a snapshot, for a truthy value that is not `true` and for a value that is not a count.

Rust: `cargo test --locked` and Clippy for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`, which embeds the dashboard (determinism, escaping and rendered-size bounds of the offline documents; asset routes).

Browser, real assets: the console over synthetic data and the hosted page, served with the production response policies; a generated catalog and report from retained journals. At 320, 768, 1440 and 1920px, light and dark (and 900px for the offline documents and the hosted page): no console error, no off-origin request, no document-level horizontal overflow, 44px touch targets below 60rem with and without JavaScript, visible focus, reduced motion, signed-out, empty and unavailable states, the running count appearing and clearing, the busy line appearing and clearing, and the saved-views list rendering. The four console tables at 63 widths from 320px to 1920px: no word of a name or title broken, no status label cut, no scroll inside a table region and no overlapping cell.

Traceability: the change's delta-spec roots are selected in `cairn-policy/development-coverage.ncl` while it is in flight, and coverage runs under the `kiln-active-development` profile. The accepted-only profile reports this change's markers as dangling until the delta is synchronised; that is the expected state of an unaccepted change, not a pass.

Not run here: the workspace-wide Cargo gates, the Octet check, the full Nix flake check, the Bombadil rails, the web-service smoke rails and deployment. The evidence names them.
