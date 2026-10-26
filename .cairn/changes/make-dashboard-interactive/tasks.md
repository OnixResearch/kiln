# Tasks: interactive dashboard

## Phase 1: Implementation

- [x] [serial] T1 Add bounded HTML-only detail projection using accepted observation views. r[kiln.observe.dashboard.html.interactive]
- [x] [parallel] T2 Render accessible search, filter, sort and run/attempt drill-down interactions with offline fragment state. r[kiln.observe.dashboard.html.interactive.scenario.navigation]
- [x] [parallel] T3 Update the HTML/browser/live contracts and operator documentation for trusted inline script and unchanged read-only boundaries. r[kiln.observe.dashboard.html.scenario.self_contained]

## Phase 2: Verification

- [x] [serial] V1 Verify bounded projections, redaction, deterministic generation and escaping with positive and negative coverage. r[kiln.observe.dashboard.html.interactive.scenario.bounded]
- [x] [serial] V2 Exercise actual generated pages in a browser and run required repository checks; record scoped results. r[kiln.observe.dashboard.html.browser_rail.scenario.match]

## Phase 3: UI and developer experience

- [x] [serial] U1 Build the responsive operator-console markup and accessible keyboard/theme interactions without changing admitted facts. r[kiln.observe.dashboard.html.console]
- [x] [parallel] U2 Apply the console visual system, responsive layouts and no-script presentation. r[kiln.observe.dashboard.html.console.scenario.responsive]
- [x] [parallel] U3 Add the representative admitted preview and document one-command generation and source rebuild workflow. r[kiln.observe.dashboard.html.preview.scenario.generate]

## Phase 4: UI and developer-experience verification

- [x] [serial] U4 Exercise real desktop/narrow, keyboard, theme, no-script, filtering, selection and watch behavior; run focused checks and record scope. r[kiln.observe.dashboard.html.console.scenario.keyboard]
