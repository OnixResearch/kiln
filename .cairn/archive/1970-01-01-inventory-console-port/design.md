## Context

`ui.js` holds the one numbered register of console destinations (`KilnUi.destinations`) that the rail, page title block, Guide and command palette read. `web.html` declares one `<section data-panel="PAGE">` per destination and one rail link per top-level destination, loads every stylesheet and script, and `server.rs` serves each asset from a fixed table. `web.js` `loadPage` dispatches each page either to its own renderer or to a page module that defines a `window.Kiln*` global (`KilnSource`, `KilnReceipts`, `KilnWorkers`, `KilnCairn`, `KilnProject`, `KilnCodeReview`, `KilnWorkUI`). Node tests in `tests/` load some of these modules in a `vm` context; pages rendered by `web.js` itself have no Node test that loads their renderer and are covered by the service smoke, the Bombadil exploration and the explorer's named examples.

## Goals and Success Contract

A maintainer porting a route can read, for every destination, which panel, renderer, stylesheets, API paths, states and checks define it today, and which rollout cohort it belongs to. The inventory cannot silently fall behind the console: adding, removing or renaming a destination, panel, module, global, stylesheet, anchor, API path or named test without updating the inventory fails a check that already runs in the repository's Nix checks.

## Decisions

### Decision: a JSON inventory checked by a Node test

**Choice:** keep the inventory as JSON beside its documentation and check it with a Node test that reads the real console sources.

**Rationale:** its correctness is a set of cross-references to source files, not a value shape a schema contract can express. The Node test already runs in the `live-console-ui` Nix check, so no new toolchain step is needed.

**Alternative:** a Nickel contract with exported JSON. It could check field shapes, but not that a module still defines its global or that a panel still exists, and it would add an export freshness step.

### Decision: cohorts follow the issue's rollout list

**Choice:** six cohorts in the issue's order: runs pilot and shared shell; work and planning; code review; projects and source; workers and inventories; receipts, feeds, administration and remaining destinations. Overview and the Guide join the first cohort because they are the landing and orientation surfaces of the shell.

**Rationale:** the rollout gates are per cohort, so each route needs exactly one.

### Decision: record check gaps instead of inventing coverage

**Choice:** each route lists `node_tests` (Node tests that load its renderer module) separately from `other_checks` (service smoke, exploration and explorer examples). The test requires every listed Node test to load the renderer it claims to cover. Pages rendered by `web.js` keep an empty `node_tests` list, and the documentation names them as needing capture evidence from #66 before their port.

**Rationale:** the inventory is the port's acceptance baseline; overstating existing coverage would let a route pass on checks that never exercised it.

## Risks / Trade-offs

- API paths are checked as strings in the renderer sources, which shows the module still consumes them but not that every consumer is listed.
- The inventory describes structure and checks, not visual parity; zero unexplained visual differences remain #66's capture gate.

## Verification Strategy

Run the new Node test with positive cases over the real console and negative cases over mutated copies of the inventory (omitted destination, unknown destination, wrong cohort, missing panel, renamed module, missing global, unserved stylesheet, absent API path, nonexistent test and a Node test that does not load the claimed renderer). Add it to the `live-console-ui` flake check, then run the Nix check, Cairn gates, validation and traceability.
