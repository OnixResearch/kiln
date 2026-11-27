# Console Port Inventory Specification

## Purpose

Defines the `console-port-inventory` capability.

## Requirements

### Requirement: Checked console port inventory

r[kiln.web.console_inventory] The repository MUST keep a machine-readable inventory of the current live console as the reference for its port. Every registered destination MUST appear exactly once, in exactly one declared rollout cohort, with its page panel, renderer module and global, page stylesheets, consumed API paths, preserved states, Node tests that load its renderer and other checks. Shared shell components MUST list their DOM anchors, owner module and stylesheets. A check that runs with the repository's Nix checks MUST fail when the inventory omits a destination, names one that is not registered, or references a panel, rail link, module, global, stylesheet, anchor, API path or test that the served console does not have.

#### Scenario: Every destination is inventoried once

r[kiln.web.console_inventory.scenario.complete]
- GIVEN the destination register and the page panels of the current console
- WHEN the inventory check runs
- THEN every destination MUST appear exactly once in one declared cohort with a matching panel and number
- AND every cohort MUST hold at least one route

#### Scenario: Drift fails the check

r[kiln.web.console_inventory.scenario.drift]
- GIVEN an inventory entry naming a panel, module, global, stylesheet, anchor, API path or test that the console no longer serves, loads or contains
- WHEN the inventory check runs
- THEN it MUST fail and name the entry and the missing reference
- AND a Node test listed for a route MUST load that route's renderer module

### Requirement: Seven-destination console inventory with legacy deep-link aliases

r[kiln.web.console_inventory_seven_destinations]
`docs/console-port-inventory.json`, `web.html`, `ui.js`, and `tests/live-console-ui.test.mjs` MUST be updated in lockstep to consolidate the primary navigation rail to 7 top-level destinations across 3 groups (`1.1 Factory`, `1.2 Attention`, `2.1 Delivery & Cairn`, `2.2 Runs & Evidence`, `2.3 Code Review & Queue`, `3.1 Fleet & Workers`, `3.2 System & Source`) plus rail footer (`0.0 Guide`, `4.4 Settings`). `web.js:readRoute()` MUST preserve backward-compatible deep-link resolution for all legacy `?page=` identifiers (`triage`, `work`, `planning`, `workviews`, `reviews`, `evidence`, `workers`, `operations`, `automation`, `activity`, `feeds`, `cobs`) by routing them to their consolidated destination and sub-tab.

#### Scenario: Legacy bookmark resolves cleanly into consolidated destination

r[kiln.web.console_inventory_seven_destinations.scenario.legacy_alias]
- GIVEN a bookmarked URL such as `?page=triage` or `?page=workers&worker_campaign=c-1`
- WHEN the console loads the URL
- THEN `readRoute()` MUST route cleanly to the consolidated destination and matching sub-tab without dropping scoped parameters
- AND `tests/console-inventory.test.mjs` MUST pass with zero drift
