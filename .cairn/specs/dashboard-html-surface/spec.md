# Dashboard Html Surface Specification

## Purpose

Defines the `dashboard-html-surface` capability.

## Requirements

### Requirement: Self-contained HTML report
r[kiln.observe.dashboard.html] The dashboard MUST render an HTML report of the same admitted views it renders in the terminal report, and that document MUST carry its style inline, MUST contain no script, and MUST reference no external resource.

#### Scenario: The HTML report states the admitted facts
r[kiln.observe.dashboard.html.scenario.parity]
- GIVEN an admitted journal or store and an admitted dashboard profile
- WHEN the dashboard renders the HTML report
- THEN the document MUST state the observed state revision, the profile identities, the declared journal identity, the recorded view identities, the redacted fields, and the declared non-claims, exactly as the terminal report states them.

#### Scenario: The document is self-contained
r[kiln.observe.dashboard.html.scenario.self_contained]
- GIVEN any rendered HTML report
- WHEN the document is inspected
- THEN it MUST contain no script, no external stylesheet, and no external resource reference, and MUST open without a network.

#### Scenario: The document is deterministic
r[kiln.observe.dashboard.html.scenario.determinism]
- GIVEN the same admitted state and the same profile
- WHEN the HTML report is rendered twice
- THEN the two documents MUST be byte-identical.

#### Scenario: Dynamic content is escaped
r[kiln.observe.dashboard.html.scenario.escaping]
- GIVEN an admitted record whose identifiers contain HTML metacharacters
- WHEN the HTML report is rendered
- THEN the document MUST contain the escaped text and MUST NOT contain the raw metacharacters as markup.

### Requirement: HTML format selection and output
r[kiln.observe.dashboard.html.format] The dashboard MUST accept an explicit report format selection of text or html, MUST default to the text report, and MUST write the selected report to standard output or to an operator-named file.

#### Scenario: The operator selects the HTML report
r[kiln.observe.dashboard.html.format.scenario.selection]
- GIVEN an operator request for the html format
- WHEN the dashboard renders
- THEN it MUST produce the HTML report and MUST reject an unsupported format name with a bounded diagnostic.

### Requirement: Browser property rail for the HTML report
r[kiln.observe.dashboard.html.browser_rail] The property rail MUST drive the rendered HTML report in a browser and MUST state that the document shows the observed facts, carries no script, and loads no external resource.

#### Scenario: The rendered document satisfies the browser rail
r[kiln.observe.dashboard.html.browser_rail.scenario.match]
- GIVEN the rendered HTML report and the browser specification
- WHEN the rail drives the document in a browser
- THEN the document MUST show the observed facts, MUST carry no script, MUST load no external resource, and the rail MUST report no violation.

### Requirement: HTML surface non-claims
r[kiln.observe.dashboard.html.nonclaims] The HTML report MUST NOT claim CI success, provider correctness, source trust, journal or store completeness, release readiness, or that the observed revision is the newest one, and the browser rail MUST NOT be presented as an exhaustive exploration.

#### Scenario: HTML evidence stays scoped
r[kiln.observe.dashboard.html.nonclaims.scenario.scope]
- GIVEN a passing browser rail run
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the checked document, the checked properties, the browser session, and the pinned tool revision.
