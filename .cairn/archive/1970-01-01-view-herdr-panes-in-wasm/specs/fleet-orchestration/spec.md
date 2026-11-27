## ADDED Requirements

### Requirement: Herdr panes render in a read-only WASM terminal viewer

r[kiln.sdlc.herdr_viewer]
Kiln MUST render an operator-selected Herdr pane's bounded ANSI snapshot in a WebAssembly terminal viewer that interprets escapes only as terminal state. The viewer MUST NOT send input to the pane, and MUST fall back to plain-text capture when the module cannot load.

#### Scenario: Operator views a colored pane

r[kiln.sdlc.herdr_viewer.scenario.view]
- GIVEN a configured host with a pane showing colored output
- WHEN an operator opens the pane in the viewer
- THEN the console renders the pane's cells with their colors
- AND refreshes only while the viewer is open and visible

#### Scenario: Hostile output stays inert

r[kiln.sdlc.herdr_viewer.scenario.inert]
- GIVEN pane output containing HTML, script text, and unsupported escape sequences
- WHEN the viewer renders it
- THEN no markup or script is interpreted
- AND unsupported sequences have no effect outside the cell grid
