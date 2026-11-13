## MODIFIED Requirements

### Requirement: Cohesive live console presentation

r[kiln.web.visual_design]
The live workspace MUST present a cohesive console in the Kiln technical graphics standard: paper and ink surfaces with solid ink action emphasis, built from one shared token layer (spacing, type, radii, elevation, motion and semantic status colors), consistent typography and clear hierarchy across CI, work and code-review views. Each view MUST expose a single primary action, with secondary and session actions visually subordinate. Light, dark, system appearance, compact density and reduced motion MUST remain functional. The Overview project list, the run list, a project's run history and the code-review change list MUST stack their rows into lines when their own region is 48rem or narrower, and a status label in them MUST be shown whole. The visual change MUST preserve existing routes, identity, state, authority, confirmation and keyboard contracts, and MUST NOT introduce remote asset requests or fabricated observations.

#### Scenario: Desktop console and appearance

- GIVEN an authenticated workspace with retained CI observations
- WHEN the operator navigates CI, work and code-review views and switches appearance or density
- THEN labels, statuses and real observations MUST remain readable with AA contrast, controls operable and semantic success/error distinct from action emphasis

#### Scenario: Narrow and keyboard interaction

- GIVEN a 320px viewport or keyboard-only navigation
- WHEN the operator opens navigation, the account menu, creation, filters or the command palette
- THEN controls MUST remain reachable, touch controls at least 44px high on mobile, focus visible, and content contained without document-level horizontal overflow

#### Scenario: Lists in a narrow region

r[kiln.web.visual_design.scenario.lists]
- GIVEN the Overview project list, the run list, a project's run history or the code-review change list at a viewport of 320px or wider
- WHEN the list's own region is 48rem or narrower: on a phone, on a portrait tablet or beside a second column
- THEN its rows MUST stack into lines instead of columns, each status label MUST be shown whole, and the document MUST NOT overflow horizontally

### Requirement: Readable observations with exact values preserved

r[kiln.web.readable_observations]
The live workspace MUST present canonical outcomes as icon-and-label status indicators, durations in human units, byte quantities in binary units and timestamps as relative times, while keeping the exact canonical outcome, recorded duration, exact byte count and absolute local and UTC instant available to assistive technology and on hover or focus. Formatting MUST be presentation-only and MUST NOT alter stored values. Timing that a run cannot have yet MUST be named from its recorded phase: the duration of a running run reads as in progress, a queued or awaiting-approval run reads as not started, and an unfinished run's completion reads as not finished. Other absent or invalid timing MUST render as not recorded, never as zero or the current time.

#### Scenario: Failed run in overview

r[kiln.web.readable_observations.scenario.failed]
- GIVEN a retained run whose canonical outcome is DomainFailure with a recorded duration of 273357 ms
- WHEN the operator views it in the overview or run list
- THEN it MUST show a failure indicator labelled Failed with duration 4m 33s, and the exact outcome DomainFailure and exact duration MUST remain inspectable

#### Scenario: Missing timing

r[kiln.web.readable_observations.scenario.unknown]
- GIVEN a retained finished, unknown or unreadable run without recorded timing
- WHEN it is rendered anywhere in the console
- THEN duration and time MUST read as not recorded, and no relative time or zero duration MUST be shown

#### Scenario: Timing a run cannot have yet

r[kiln.web.readable_observations.scenario.pending]
- GIVEN a running run without a recorded duration, and a queued or awaiting-approval run without a recorded start
- WHEN either is rendered in the run list, the run header, the Overview project list or a project's run history
- THEN the running run's duration MUST read In progress, the waiting run's duration and start MUST read Not started, neither completion MUST read as recorded, a recorded value MUST still be shown as recorded, and no elapsed time, zero duration or current time MUST be substituted

## ADDED Requirements

### Requirement: Numbered wayfinding

r[kiln.web.wayfinding]
The live console MUST keep one numbered register of its destinations: each destination MUST have one section number in a numbered group, a title and a one-sentence purpose, and a page that belongs to another destination (a project's history under CI projects) MUST share that destination's number. The navigation rail, the page title block, the breadcrumb, the command palette's "Go to" commands and the Guide MUST read their numbers, titles and purposes from that register, and the console MUST admit exactly the register's pages. A Guide page MUST list every destination by number with its purpose, how each run state reads, the words Kiln uses and the keyboard reference, with a short first path for a new operator. The brand MUST return to the page that a bare console URL opens. An empty workspace MUST name its next step. When the rail scrolls, the current destination MUST be scrolled fully into the rail's own view without moving the page. A section number MUST NOT be part of a navigation link's accessible name.

#### Scenario: A destination reads the same everywhere

r[kiln.web.wayfinding.scenario.register]
- GIVEN the destination register
- WHEN the rail, the page title block, the breadcrumb, the command palette and the Guide are shown for a destination
- THEN each MUST show that destination's number and title, the title block MUST show its group and purpose, typing the number in the command palette MUST offer that destination, and no two top-level destinations MUST share a number

#### Scenario: A new operator orients

r[kiln.web.wayfinding.scenario.orientation]
- GIVEN an authenticated workspace with no enrolled project
- WHEN the operator opens the console
- THEN the Overview MUST offer CI projects and the Guide as next steps, and the Guide MUST explain every run state the console can show before any run exists
