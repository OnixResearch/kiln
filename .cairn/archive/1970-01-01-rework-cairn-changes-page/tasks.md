## Phase 1: Projection

- [x] [serial] Derive unit and change lanes, lane counts and open decisions, rank blockers with their subject kind and campaign, add each work version's current dependents and move both documents to `v2`, with positive and negative cases. r[kiln.cairn.workspace.scenario.lanes] r[kiln.cairn.workspace.scenario.blockers] r[kiln.cairn.workspace.scenario.dependencies]

## Phase 2: Console

- [x] [serial] Split the campaign decision answer out of `inventory.js` and export it with the inbox reader and command runner, leaving the campaign view unchanged. r[kiln.cairn.workspace.scenario.decisions]
- [x] [serial] Rewrite the register in `cairn.js` and `cairn.css` as a list and a board with text, lane, blocker and decision filters, counts, arrow-key movement and URL restoration, and parse its route keys in `web.js`. r[kiln.cairn.workspace.scenario.register] r[kiln.cairn.workspace.scenario.reload]
- [x] [serial] Rewrite the change detail with next steps, inline decisions, the URL-bound unit matrix and attempt timeline, the dependency graph, campaign members, an identities disclosure and previous and next navigation. r[kiln.cairn.workspace.scenario.detail] r[kiln.cairn.workspace.scenario.decisions] r[kiln.cairn.workspace.scenario.dependencies]
- [x] [serial] Add positive and negative Node cases for schema admission, lane words, ordering, filters, neighbors, unit routes, graph layering, late responses and inline decision admission. r[kiln.cairn.workspace.scenario.register] r[kiln.cairn.workspace.scenario.reload] r[kiln.cairn.workspace.scenario.decisions] r[kiln.cairn.workspace.scenario.dependencies]

## Phase 3: Verification

- [x] [serial] Assert the exploration example's lanes, then run the crate Cargo tests, rustfmt, Clippy, JS syntax checks and the Node rail and record their scope. r[kiln.cairn.workspace.scenario.example] r[kiln.cairn.workspace]
- [x] [serial] Review real assets over the exploration workspace at desktop, tablet and phone widths in light and dark, covering filters, board, keyboard, next steps, an inline answer, unit expansion, the graph and campaign links, and record bounded evidence. r[kiln.cairn.workspace.scenario.detail] r[kiln.cairn.workspace.scenario.register]
- [x] [serial] Update the operator guide, console inventory and changelog, register the active delta root, and pass Cairn validation and traceability. r[kiln.cairn.workspace]

Verification is recorded in `evidence/verification.json`; the declared-coverage
receipt for both traceability profiles is in `evidence/traceability.json`.
Documentation, console inventory and the active delta root are updated; Cairn
validation passes. The traceability gate now reports no missing or dangling
requirement in either the accepted `cairn-default` profile or the
`kiln-active-development` profile.

The earlier two missing GitHub requirements were retired by the
`retire-removed-github-mutations` change. The dangling campaign projection
marker was resolved by rewriting the active `selected-cairn-campaign` delta as
explicit added requirements and syncing it. Syncing every active delta exposed
accepted campaign, campaign-projection and native-Git requirements that carried
no reference anchor; each was anchored at its existing implementation or test
site. No requirement text, runtime behavior or default policy changed for those
capabilities. Declared reference coverage is not behavioral conformance or
release eligibility.
