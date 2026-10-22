# Tasks: add an HTML report to the operator dashboard

## Phase 1: Renderer and options

- [x] [serial] T1 Add the self-contained HTML renderer over the admitted report. r[kiln.observe.dashboard.html]
- [x] [parallel] T2 Add the format selection and output path to the dashboard shell and the operator CLI. r[kiln.observe.dashboard.html.format]
- [x] [parallel] T3 Add the tests that bind the HTML report to the terminal report, assert determinism, self-containment, and escaping. r[kiln.observe.dashboard.html.scenario.parity] r[kiln.observe.dashboard.html.scenario.determinism] r[kiln.observe.dashboard.html.scenario.escaping]

## Phase 2: Browser rail

- [x] [serial] T4 Write the browser specification that states the observed facts, the absence of script, and the absence of external resources. r[kiln.observe.dashboard.html.browser_rail]
- [x] [parallel] T5 Check in the browser specification and record the command that runs it. It is not a Nix check: the build sandbox gives the browser no reachable loopback, and the builder refuses a derivation that asks to run without the sandbox. r[kiln.observe.dashboard.html.browser_rail.scenario.match]

## Phase 3: Documentation and completion evidence

- [x] [serial] T6 Document the HTML report, its options, and its non-claims. r[kiln.observe.dashboard.html.nonclaims]
- [x] [serial] V1 Run the crate tests, the workspace gates, the property rail, and the Cairn gates. r[kiln.observe.dashboard.html.scenario.self_contained]
- [x] [serial] V2 Record the evidence, sync the accepted spec, archive the change, and publish the tip with receipts. r[kiln.observe.dashboard.html.nonclaims.scenario.scope]
