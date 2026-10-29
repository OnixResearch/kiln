## ADDED Requirements

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
