## MODIFIED Requirements

### Requirement: Cohesive live console presentation

r[kiln.web.visual_design]
The live workspace MUST present a cohesive off-white/charcoal console with acid-yellow action emphasis, built from one shared token layer (spacing, type, radii, elevation, motion and semantic status colors), consistent typography and clear hierarchy across CI, work and code-review views. Each view MUST expose a single primary action, with secondary and session actions visually subordinate. Light, dark, system appearance, compact density and reduced motion MUST remain functional. The visual change MUST preserve existing routes, identity, state, authority, confirmation and keyboard contracts, and MUST NOT introduce remote asset requests or fabricated observations.

#### Scenario: Desktop console and appearance

- GIVEN an authenticated workspace with retained CI observations
- WHEN the operator navigates CI, work and code-review views and switches appearance or density
- THEN labels, statuses and real observations MUST remain readable with AA contrast, controls operable and semantic success/error distinct from action emphasis

#### Scenario: Narrow and keyboard interaction

- GIVEN a 320px viewport or keyboard-only navigation
- WHEN the operator opens navigation, the account menu, creation, filters or the command palette
- THEN controls MUST remain reachable, touch controls at least 44px high on mobile, focus visible, and content contained without document-level horizontal overflow

## ADDED Requirements

### Requirement: Readable observations with exact values preserved

r[kiln.web.readable_observations]
The live workspace MUST present canonical outcomes as icon-and-label status indicators, durations in human units and timestamps as relative times, while keeping the exact canonical outcome, recorded duration and absolute local and UTC instant available to assistive technology and on hover or focus. Formatting MUST be presentation-only and MUST NOT alter stored values. Absent or invalid timing MUST render as not recorded, never as zero or the current time.

#### Scenario: Failed run in overview

r[kiln.web.readable_observations.scenario.failed]
- GIVEN a retained run whose canonical outcome is DomainFailure with a recorded duration of 273357 ms
- WHEN the operator views it in the overview or run list
- THEN it MUST show a failure indicator labelled Failed with duration 4m 33s, and the exact outcome DomainFailure and exact duration MUST remain inspectable

#### Scenario: Missing timing

r[kiln.web.readable_observations.scenario.unknown]
- GIVEN a retained run without recorded timing
- WHEN it is rendered anywhere in the console
- THEN duration and time MUST read as not recorded, and no relative time or zero duration MUST be shown
