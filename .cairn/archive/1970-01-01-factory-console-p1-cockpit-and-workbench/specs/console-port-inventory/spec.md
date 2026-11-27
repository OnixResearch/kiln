## ADDED Requirements

### Requirement: Seven-destination console inventory with legacy deep-link aliases

r[kiln.web.console_inventory_seven_destinations]
`docs/console-port-inventory.json`, `web.html`, `ui.js`, and `tests/live-console-ui.test.mjs` MUST be updated in lockstep to consolidate the primary navigation rail to 7 top-level destinations across 3 groups (`1.1 Factory`, `1.2 Attention`, `2.1 Delivery & Cairn`, `2.2 Runs & Evidence`, `2.3 Code Review & Queue`, `3.1 Fleet & Workers`, `3.2 System & Source`) plus rail footer (`0.0 Guide`, `4.4 Settings`). `web.js:readRoute()` MUST preserve backward-compatible deep-link resolution for all legacy `?page=` identifiers (`triage`, `work`, `planning`, `workviews`, `reviews`, `evidence`, `workers`, `operations`, `automation`, `activity`, `feeds`, `cobs`) by routing them to their consolidated destination and sub-tab.

#### Scenario: Legacy bookmark resolves cleanly into consolidated destination

r[kiln.web.console_inventory_seven_destinations.scenario.legacy_alias]
- GIVEN a bookmarked URL such as `?page=triage` or `?page=workers&worker_campaign=c-1`
- WHEN the console loads the URL
- THEN `readRoute()` MUST route cleanly to the consolidated destination and matching sub-tab without dropping scoped parameters
- AND `tests/console-inventory.test.mjs` MUST pass with zero drift
