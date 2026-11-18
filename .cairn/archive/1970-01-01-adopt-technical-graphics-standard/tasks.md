## Phase 1: Standard

- [x] [serial] Redeclare `identity.css` to the technical graphics standard: paper and ink neutrals with `--frame`, ink action emphasis and focus, identity red with the ink and amber door, signal-light statuses, local grotesque and monospace stacks with the label tokens, zero radius tiers, unblurred overlay offsets and the ink title rule; remove the ember tokens and their uses; keep the appearance override, reduced motion, selection and focus rules; redraw the favicon. r[kiln.identity.graphics_standard] r[kiln.identity.contrast]
- [x] [serial] Extend the identity check: frame, selected-row and brand-mark pairs, the red/failure distance, and the standard check for identity red, radius and shadow over every surface stylesheet, with failing fixtures and an allowed fixture. r[kiln.identity.graphics_standard.scenario.surfaces] r[kiln.identity.graphics_standard.scenario.regression] r[kiln.identity.contrast.scenario.palette] r[kiln.identity.contrast.scenario.regression]

## Phase 2: Surfaces

- [x] [serial] Restyle the console shell and shared components on the standard: rail, top bar, title block, panels, tables, tabs, chips, forms, dialogs, command palette, run inspector and log panel; restore pressed states in the flat grammar. Keep every route, label of record, confirmation and keyboard path. r[kiln.web.visual_design]
- [x] [serial] Restyle the work, code-review, project and source pages, the offline catalog and report, and the hosted bridge page on the standard. r[kiln.identity.graphics_standard] r[kiln.web.visual_design]

## Phase 3: Wayfinding and readable observations

- [x] [serial] Add the numbered destination register to `ui.js`; read it in the rail, title block, breadcrumb, document title, page admission and command palette; move the CI group first; return the brand to Overview; keep the current destination in the rail's view; test the register's invariants. r[kiln.web.wayfinding] r[kiln.web.wayfinding.scenario.register]
- [x] [serial] Add the Guide page from the register, the status rule and the review shortcut table; link it from the rail footer, Settings and the empty Overview. r[kiln.web.wayfinding] r[kiln.web.wayfinding.scenario.orientation]
- [x] [serial] Name timing a run cannot have yet in the run list, the run header, the Overview project list and a project's run history, and test running, waiting, recorded, finished, unknown and malformed runs. r[kiln.web.readable_observations] r[kiln.web.readable_observations.scenario.pending] r[kiln.web.readable_observations.scenario.unknown]
- [x] [serial] Move `humanBytes` into `ui.js`; show retention quantities in binary units with the exact count and share of the limit; lead CI projects with the inventory; state run moments on project cards; give worker badges signal tones; render custom work statuses as neutral cells. r[kiln.web.readable_observations]

## Phase 4: Verification

- [x] [serial] Run the node rail; `cargo test --locked` for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`, which embed the changed stylesheets and scripts (no Rust source changes, so `cargo fmt` and Clippy cannot differ from the parent revision); the lifecycle validation; and traceability coverage under `kiln-active-development` with this change's delta-spec roots selected. r[kiln.identity.graphics_standard] r[kiln.web.wayfinding] r[kiln.web.readable_observations]
- [x] [serial] Review the real assets in a browser over synthetic data: every console page in light and dark at 1440px, the explicit appearance override, focus, the command palette's number search, the empty and signed-out states, 390px and 768px for overflow and 44px targets, the hosted page in both appearances, and the catalog and report rendered from the working tree. r[kiln.web.visual_design] r[kiln.identity.graphics_standard.scenario.surfaces]
- [x] [serial] Update the operator and dashboard documentation and the changelog, and retain bounded evidence that names every rail not run. r[kiln.web.visual_design] r[kiln.web.wayfinding]

`evidence/verification.json` holds the checks run on this source tree and names
what was not run; `evidence/lifecycle.json` holds the planning-gate, validation
and traceability results observed while the change is in flight. Acceptance,
synchronisation into the accepted specifications, archive, commit, publication
and deployment remain the operator's decisions.
