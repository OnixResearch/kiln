## Phase 1: Identity layer

- [x] [serial] Add `identity.css` with every colour token as `light-dark()`, the type stacks, scales, brand mark and heading rule; include it in the report, catalog, console and hosted page; move `catalog.css`, `html.css` and the hosted stylesheet to the semantic token names; remove the mapping block, restated rules and duplicated dark blocks. r[kiln.identity.shared] r[kiln.identity.shared.scenario.surfaces]
- [x] [serial] Add the identity check to the node rail: contrast floor in both appearances, palette ownership and token definition, with a passing case over the real stylesheets and failing fixtures for a lowered colour, a palette literal and an undefined token. r[kiln.identity.contrast] r[kiln.identity.contrast.scenario.palette] r[kiln.identity.contrast.scenario.regression] r[kiln.identity.shared.scenario.ownership]

## Phase 2: Surfaces

- [x] [serial] Restyle the live console shell and shared components on the identity: dark rail, top bar, page heading rule, buttons, inputs, panels, status indicators, tables, tabs, menus, dialogs, sign-in, empty, loading and unavailable states, run inspector and log panel. Keep every route, label, control, confirmation and keyboard path. r[kiln.web.visual_design]
- [x] [serial] Restyle the offline catalog and report on the identity, with the new overview hero, the restated index-widget colours and an outcome column that holds the longest outcome name; keep determinism, escaping, no-script reading and rendered-size bounds. r[kiln.identity.shared] r[kiln.identity.shared.scenario.surfaces]
- [x] [serial] Restyle the hosted bridge page on the identity in light and dark under its existing response policy; let a table of record rows wider than its panel scroll inside a labelled, focusable region that is a tab stop only while it holds rows. r[kiln.identity.shared] r[kiln.identity.shared.scenario.surfaces]
- [x] [serial] Give the offline catalog, the offline report and the hosted bridge page one touch layout at 960px and narrower: buttons, links, fields, selects and disclosure headings at least 44px tall in both densities, with and without JavaScript, with the catalog's phone top bar still fitting at 320px. r[kiln.identity.touch_targets] r[kiln.identity.touch_targets.scenario.narrow]
- [x] [serial] Draw the focus ring of a focusable dark region (hosted response panel, catalog index table) inside its own edge, so the ring reaches 3:1 on the surface it sits on in light appearance. r[kiln.identity.contrast]

## Phase 3: Console UX

- [x] [serial] Add `KilnUi.liveActivity` and `KilnUi.presentedRunning`, the top-bar running link and the lit mark; hide both while observation is stale and after the session ends; test running, queued, awaiting-approval, unknown, finished, malformed and non-array snapshots, and current, stale, absent and non-count presentation inputs. r[kiln.web.live_activity] r[kiln.web.live_activity.scenario.count] r[kiln.web.live_activity.scenario.stale]
- [x] [serial] Add the request counter at the fetch boundary, the delayed top-bar progress line and the busy state for assistive technology; exclude the live-observation poll and log following; clear on success, failure, timeout and abort. r[kiln.web.request_feedback] r[kiln.web.request_feedback.scenario.slow]
- [x] [serial] Add the account-menu appearance switch and the `/favicon.svg` route, and repair the saved-views crash in the code-review list and the status lane headings of work boards. r[kiln.web.visual_design]
- [x] [serial] Stack the Overview project list, a project's run history, the code-review change list and the run list by the width of their own region (48rem; one fact per line and a status line at 24rem), so no status label is cut and no ordinary word of a project name or change title is broken. r[kiln.web.visual_design] r[kiln.web.visual_design.scenario.lists]

## Phase 4: Verification

- [x] [serial] Run the node rail, `cargo fmt --check`, `cargo test --locked` and Clippy for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`, the dashboard publication and refresh tests against the built executable, the lifecycle validation, and traceability coverage under the `kiln-active-development` profile with this change's delta-spec roots selected. r[kiln.identity.shared] r[kiln.identity.contrast] r[kiln.web.live_activity] r[kiln.web.request_feedback]
- [x] [serial] Review the real assets in a browser at 320, 768, 1440 and 1920px in light and dark: console over synthetic data, hosted page, generated catalog and report. Check console errors, off-origin requests, horizontal overflow, 44px touch targets with and without JavaScript, the focus ring of each focusable element on the reviewed pages, reduced motion, signed-out, empty and unavailable states, the running count appearing and clearing on stale observation, the progress line appearing and clearing after a failed read, and the console tables at 63 widths for broken words and cut status labels. r[kiln.web.visual_design] r[kiln.identity.shared.scenario.surfaces] r[kiln.identity.touch_targets.scenario.narrow] r[kiln.web.live_activity.scenario.stale] r[kiln.web.request_feedback.scenario.slow]
- [x] [serial] Update the operator, dashboard and hosted documentation and the changelog, and retain bounded evidence that names every rail not run. r[kiln.web.visual_design] r[kiln.identity.shared]

`evidence/verification.json` holds the checks run on this source tree and names
what was not run; `evidence/lifecycle.json` holds the planning-gate and
validation results observed while the change was in flight. On 2026-11-13 the
operator accepted the change: its delta was synchronized into the accepted
specifications and the change archived. `evidence/acceptance.json` holds the
synchronization and archive receipts and the traceability coverage and release
readiness results that follow. Publication and deployment are recorded after
the fact.
