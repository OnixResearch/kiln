# Live Console Design Specification

## Purpose

Defines the `live-console-design` capability.

## Requirements

### Requirement: Cohesive live console presentation

r[kiln.web.visual_design]
The live workspace MUST present a cohesive console in the Kiln identity: warm porcelain and soot surfaces with ember action emphasis, built from one shared token layer (spacing, type, radii, elevation, motion and semantic status colors), consistent typography and clear hierarchy across CI, work and code-review views. Each view MUST expose a single primary action, with secondary and session actions visually subordinate. Light, dark, system appearance, compact density and reduced motion MUST remain functional. The Overview project list, the run list, a project's run history and the code-review change list MUST stack their rows into lines when their own region is 48rem or narrower, and a status label in them MUST be shown whole. The visual change MUST preserve existing routes, identity, state, authority, confirmation and keyboard contracts, and MUST NOT introduce remote asset requests or fabricated observations.

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

### Requirement: Structured console layout

r[kiln.web.console_layout]
The live workspace MUST use a compact icon rail whose labels stay available to assistive technology and appear on hover, keyboard focus or pinning, a slim top bar, and a centred page frame with a prominent page header. The CI overview MUST lead with failing projects and summarize health, worker capacity and recent activity using only existing bounded read APIs, with an explicit loading, empty or unavailable state per section. On wide screens the run view MUST show pipeline and logs together. All existing routes, controls, confirmations and keyboard behaviour MUST remain available.

#### Scenario: Failing project from the dashboard

r[kiln.web.console_layout.scenario.attention]
- GIVEN retained failing and passing project runs
- WHEN the operator opens the overview
- THEN failing projects MUST appear first as attention items whose Logs action opens that exact execution, and the pass rate MUST state its denominator without counting unknown runs

#### Scenario: Rail and narrow screens

r[kiln.web.console_layout.scenario.navigation]
- GIVEN keyboard-only use or a 320px viewport
- WHEN the operator moves through navigation and opens the run view
- THEN every navigation label MUST be reachable and announced, Escape MUST collapse an expanded rail or drawer, focus MUST stay visible, and no document-level horizontal overflow MUST occur

#### Scenario: Unavailable dashboard section

r[kiln.web.console_layout.scenario.unavailable]
- GIVEN the workers or activity read fails
- WHEN the overview renders
- THEN that section MUST show an explicit unavailable state while other sections remain usable, and no sample data MUST be shown

### Requirement: Consistent, readable console pages

r[kiln.web.page_consistency]
Every live-console page MUST use the shared page frame and components. Each page MUST present one clear purpose and at most one primary action, with readable primary text and exact identifiers available but secondary. Explicit loading, empty, unavailable and error states MUST be shown for each data section. Boundary and non-claim statements MUST remain reachable, and MAY be placed behind a labelled disclosure when they would otherwise dominate the page. At 320, 768, 1440 and 1920px, in light and dark appearance, pages MUST avoid document-level horizontal overflow and meaning-hiding truncation. They MUST also keep visible focus and 44px mobile touch targets.

#### Scenario: Secondary page with retained data

r[kiln.web.page_consistency.scenario.data]
- GIVEN retained CI, work or review data
- WHEN the operator opens any console page at any supported width or appearance
- THEN the page MUST show its title, purpose and primary action first, render data with shared components, and keep exact identities and boundary statements reachable

#### Scenario: Empty or unavailable page section

r[kiln.web.page_consistency.scenario.states]
- GIVEN an empty, unconfigured or failed data source
- WHEN its page renders
- THEN the affected section MUST show an explicit state with guidance where possible, and no sample data or substituted selection MUST appear

### Requirement: GPUI Kit design system in the live console

r[kiln.web.design_system]
The live console MUST draw every icon from the Lucide set of a pinned GPUI Kit revision, served from Kiln's own origin. The console's stylesheets MUST use a single semantic token vocabulary aligned with GPUI Kit's theme names, with no legacy token aliases. Pages MUST conform to GPUI Kit's Design Guides, except for documented web deviations. Navigation links, 44px mobile touch targets, visible focus and the existing route, authority and confirmation contracts MUST be preserved.

#### Scenario: Reproducible icon provenance

r[kiln.web.design_system.scenario.provenance]
- GIVEN the pinned GPUI Kit revision and the reviewed icon list
- WHEN the import script runs twice, or runs against a different revision or tampered source
- THEN the generated icon data and license MUST be byte-identical across runs, and a mismatched revision, NAR identity or missing icon MUST be refused before any file changes

#### Scenario: Icons without remote requests

r[kiln.web.design_system.scenario.local]
- GIVEN any console page
- WHEN it renders in a browser
- THEN every icon MUST render from local data, and no request MUST leave the console origin

### Requirement: Truthful live activity signal

r[kiln.web.live_activity]
On every page of an authenticated session, the live console MUST state how many retained runs the last admitted live snapshot reports as running, as text that links to the pending-run list. It MAY light the brand mark while that count is above zero. The signal MUST derive only from the last admitted live snapshot. It MUST show no count before a snapshot is admitted, none while live observation is stale or disconnected and none after the session ends. It MUST NOT count queued, awaiting-approval, unknown or finished runs as running. A reduced-motion preference MUST remove the animation without removing the text.

#### Scenario: Runs become active and finish

r[kiln.web.live_activity.scenario.count]
- GIVEN an authenticated console whose live snapshot reports two running runs, one queued run and one run awaiting approval
- WHEN any page is shown, and again after a snapshot in which no run is running
- THEN the top bar MUST first read 2 running and link to the pending runs, and MUST then show no running count

#### Scenario: Stale observation

r[kiln.web.live_activity.scenario.stale]
- GIVEN a displayed running count
- WHEN live observation fails or the session is cleared
- THEN the count and the lit mark MUST be removed, and MUST stay removed until live observation is current again

### Requirement: Feedback for requests in flight

r[kiln.web.request_feedback]
While a read or mutation request other than a background observation poll (the live-observation poll and log following) stays in flight beyond a brief delay, the live console MUST show a non-blocking progress indication in the top bar and MUST expose the busy state to assistive technology. It MUST remove the indication when no such request remains, whether the request succeeded, failed, timed out or was aborted. The indication MUST NOT imply completion, success or progress toward a known total.

#### Scenario: A slow read and its completion

r[kiln.web.request_feedback.scenario.slow]
- GIVEN an authenticated console and a read that takes longer than the delay
- WHEN the operator opens a page and the read later settles or fails
- THEN the progress indication MUST appear while the read is in flight and MUST be gone afterwards, and the live-observation poll alone MUST NOT show it
