# Dashboard Rats Index Specification

## Purpose

Defines the `dashboard-rats-index` capability.

## Requirements

### Requirement: Actual bounded Rats widget rendering
r[kiln.dashboard.rats.widget]
The primary browser catalog index MUST render actual pinned Subwayrat widgets through Ratzilla, over only the bounded admitted catalog, without introducing CI decisions or provider effects.

#### Scenario: Interactive widget selection
r[kiln.dashboard.rats.widget.scenario.selection]
- GIVEN an admitted catalog containing multiple repository/history identities
- WHEN the operator navigates the Rats index with keyboard or pointer
- THEN the selected existing identity MUST open its independently admitted report; arbitrary or absent pairs MUST NOT substitute another report.

### Requirement: Offline self-contained delivery
r[kiln.dashboard.rats.offline]
Generated JavaScript and WASM MUST be embedded in the bounded single HTML artifact. Runtime MUST NOT fetch modules, fonts, data or WASM; accessible HTML navigation and readable reports MUST survive unavailable JavaScript or WASM with visible initialization failure.

#### Scenario: File loading and unavailable browser capability
r[kiln.dashboard.rats.offline.scenario.fallback]
- GIVEN a generated report opened as a local file, with network blocked or WASM unavailable
- WHEN the browser initializes the index
- THEN supported offline initialization MUST work without external requests, and unavailable capability MUST leave readable accessible navigation without pretending Rats initialized.

### Requirement: Pinned reproducible browser inputs
r[kiln.dashboard.rats.provenance]
Browser source, dependencies, imported upstream identities and licenses MUST be explicit and locked, with maintained asset regeneration and drift checking. Ordinary Kiln builds MUST NOT require sibling paths, private ratcore or operator credentials.

#### Scenario: Source and asset drift
r[kiln.dashboard.rats.provenance.scenario.drift]
- GIVEN the recorded exact upstream revision and locked browser build inputs
- WHEN source import or asset checking runs
- THEN unchanged inputs MUST reproduce or verify the recorded artifacts and changed inputs MUST fail visibly rather than silently replacing trusted inputs.

### Requirement: Preserve catalog behavior and report isolation
r[kiln.dashboard.rats.behavior]
The browser index MUST preserve exact repository/history identities, saved per-history view state, explicit missing-selection behavior, label escaping and sandboxed report isolation at desktop and narrow viewports.

#### Scenario: Restored missing or hostile selections
r[kiln.dashboard.rats.behavior.scenario.restore]
- GIVEN a saved selection, a removed history, or labels containing markup
- WHEN the report is loaded or navigated
- THEN restoration MUST retain exact available selections, unavailable selections MUST remain visibly missing without replacement, and labels MUST remain inert text.

### Requirement: Scoped integration evidence
r[kiln.dashboard.rats.claims]
Verification MUST exercise the real compiled browser path, retain source/tool and observed outcome evidence, and document maintenance and repeatability. Passing checks MUST NOT imply CI success, arbitrary widget compatibility, hostile-runtime confinement, deployment or release eligibility; independent Patchgate holds MUST remain unchanged.

#### Scenario: Exercised browser proof
r[kiln.dashboard.rats.claims.scenario.scope]
- GIVEN compiled pinned assets and admitted fixture or retained canonical inputs
- WHEN browser and repository checks are recorded
- THEN only exercised inputs and controls MUST be claimed and unrelated failures or holds MUST remain visible.
