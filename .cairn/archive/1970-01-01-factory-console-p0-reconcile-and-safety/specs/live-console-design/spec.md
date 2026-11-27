## ADDED Requirements

### Requirement: Inventory-safe Factory rail promotion

r[kiln.web.factory_p0_rail_promotion]
The live console MUST promote `2.7 Cairn changes` and `2.8 Fleet` out of collapsed navigation disclosures into visible primary rail items while folding `kiln-ui-simplify` and source-bound owner projection groundwork cleanly. During P0, all 23 registered destination identifiers and `parent: null` registrations MUST remain intact so `docs/console-port-inventory.json`, `tests/console-inventory.test.mjs`, and `tests/live-console-ui.test.mjs` pass without drift.

#### Scenario: Factory destinations are immediately visible without breaking inventory

r[kiln.web.factory_p0_rail_promotion.scenario.visible]
- GIVEN the live console navigation rail in `web.html`
- WHEN an operator loads the console and runs the console inventory and UI tests
- THEN `Cairn changes` and `Fleet` MUST be visible without opening `More work`
- AND every registered destination MUST still match `docs/console-port-inventory.json`

### Requirement: Guarded Fleet control dialogs without blocking browser prompts

r[kiln.web.factory_p0_control_safety]
Fleet mutations in the web console (`Renew`, `Release`, `Cancel`) MUST NOT call `window.prompt()` or `window.alert()`, and MUST NOT execute destructive claim release or attempt cancellation without an explicit confirmation dialog capturing reason context and validating observation freshness against `CONTROL_FRESHNESS_MS`.

#### Scenario: Stale or unconfirmed Fleet mutation is blocked

r[kiln.web.factory_p0_control_safety.scenario.guarded]
- GIVEN an operator invoking `Release` on a claim or `Cancel` on an attempt in `fleet.js`
- WHEN the observation snapshot is older than `CONTROL_FRESHNESS_MS` or the operator has not confirmed in `#control-dialog`
- THEN the browser MUST NOT issue the mutation request
- AND the dialog MUST require a fresh snapshot and explicit confirmation before submission
