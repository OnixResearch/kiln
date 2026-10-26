# Dashboard Html Surface Specification

## Purpose

Defines the `dashboard-html-surface` capability.

## Requirements

### Requirement: Self-contained HTML report
r[kiln.observe.dashboard.html] The dashboard MUST render the admitted terminal-report facts and bounded additional drill-down views as deterministic self-contained HTML with inline style and fixed trusted inline script, and MUST reference no external resource.

#### Scenario: The HTML report states the admitted facts
r[kiln.observe.dashboard.html.scenario.parity]
- GIVEN an admitted journal or store and profile
- WHEN HTML is rendered
- THEN observed revision, profile and journal facts, exact view identities, redactions and non-claims MUST remain visible.

#### Scenario: The document is self-contained
r[kiln.observe.dashboard.html.scenario.self_contained]
- GIVEN a rendered document
- WHEN opened offline
- THEN its controls MUST work without external resources, network requests, dynamic code loading or canonical mutation.

#### Scenario: The document is deterministic
r[kiln.observe.dashboard.html.scenario.determinism]
- GIVEN identical admitted inputs
- WHEN rendered twice
- THEN output bytes MUST match.

#### Scenario: Dynamic content is escaped
r[kiln.observe.dashboard.html.scenario.escaping]
- GIVEN identifiers containing markup metacharacters
- WHEN rendered and selected
- THEN identifiers MUST remain inert text, not executable markup or selector syntax.

### Requirement: HTML format selection and output
r[kiln.observe.dashboard.html.format] The dashboard MUST accept an explicit report format selection of text or html, MUST default to the text report, and MUST write the selected report to standard output or to an operator-named file.

#### Scenario: The operator selects the HTML report
r[kiln.observe.dashboard.html.format.scenario.selection]
- GIVEN an operator request for the html format
- WHEN the dashboard renders
- THEN it MUST produce the HTML report and MUST reject an unsupported format name with a bounded diagnostic.

### Requirement: Browser property rail for the HTML report
r[kiln.observe.dashboard.html.browser_rail] The browser rail MUST check admitted facts, local interactive controls and absence of external resources or effects.

#### Scenario: The rendered document satisfies the browser rail
r[kiln.observe.dashboard.html.browser_rail.scenario.match]
- GIVEN the generated dashboard
- WHEN driven in a browser
- THEN filtering and selection MUST preserve admitted facts and identities and MUST NOT load external resources.

### Requirement: HTML surface non-claims
r[kiln.observe.dashboard.html.nonclaims] The HTML report MUST NOT claim CI success, provider correctness, source trust, journal or store completeness, release readiness, or that the observed revision is the newest one, and the browser rail MUST NOT be presented as an exhaustive exploration.

#### Scenario: HTML evidence stays scoped
r[kiln.observe.dashboard.html.nonclaims.scenario.scope]
- GIVEN a passing browser rail run
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the checked document, the checked properties, the browser session, and the pinned tool revision.

### Requirement: Interactive bounded navigation
r[kiln.observe.dashboard.html.interactive] The HTML page MUST support run search, exact outcome filtering, sorting and run/attempt detail selection over admitted bounded projections, without adding control authority.

#### Scenario: Navigate admitted runs and attempts
r[kiln.observe.dashboard.html.interactive.scenario.navigation]
- GIVEN a generated dashboard with multiple runs
- WHEN search, filter, sorting or selection changes
- THEN displayed rows and details MUST match those controls, missing targets MUST be reported, and fragment state MUST survive reload without selecting a different identity silently.

#### Scenario: Detail projection remains bounded and redacted
r[kiln.observe.dashboard.html.interactive.scenario.bounded]
- GIVEN more history than the admitted run or attempt limits and a redacting profile
- WHEN HTML drill-downs are built
- THEN only listed runs, bounded listed attempts, active attempts and explicit selections MUST have detail views, with unchanged canonical view identities, redaction and truncation facts.

### Requirement: Accessible operator-console presentation
r[kiln.observe.dashboard.html.console] The HTML surface MUST prioritize admitted summary, bounded run navigation and adjacent selected detail on desktop, retain usable narrow and no-script layouts, and expose provenance, redactions and non-claims without external resources.

#### Scenario: Operate the console with keyboard and theme controls
r[kiln.observe.dashboard.html.console.scenario.keyboard]
- GIVEN an admitted report opened offline
- WHEN an operator uses search shortcuts, shortcut help, section navigation or explicit theme selection
- THEN native keyboard focus MUST remain usable, editable controls MUST NOT have typing intercepted, fragment selection MUST remain exact and theme MUST survive reload.

#### Scenario: Inspect a narrow or no-script report
r[kiln.observe.dashboard.html.console.scenario.responsive]
- GIVEN the console at a narrow viewport or with JavaScript disabled
- WHEN its admitted facts are inspected
- THEN the document MUST NOT overflow horizontally, admitted facts and disclosures MUST remain readable, and unavailable enhanced controls MUST NOT appear functional.

### Requirement: Repeatable local developer preview
r[kiln.observe.dashboard.html.preview] A documented developer command MUST generate representative canonical sample input and the actual admitted HTML report without an external service or new frontend dependency.

#### Scenario: Generate and rebuild the local preview
r[kiln.observe.dashboard.html.preview.scenario.generate]
- GIVEN the repository toolchain and a writable output path
- WHEN the documented preview command runs and is rerun after a presentation source change
- THEN its journal MUST pass canonical admission, mixed outcomes and retry history MUST be inspectable, and the regenerated HTML MUST use the current renderer.
