# Dashboard Live Html Report Specification

## Purpose

Defines the `dashboard-live-html-report` capability.

## Requirements

### Requirement: Live HTML report
r[kiln.observe.dashboard.html.live_update] The watched HTML report MUST follow the admitted input at the declared watch interval using atomically replaced self-contained documents.

#### Scenario: A watched report follows the input
r[kiln.observe.dashboard.html.live_update.scenario.match]
- GIVEN a watched HTML output
- WHEN the shell renders a frame
- THEN it MUST atomically replace the document and carry a reload instruction derived from the profile interval; local interactive state MUST survive through its fragment.

#### Scenario: An unwatched report stays a snapshot
r[kiln.observe.dashboard.html.live_update.scenario.snapshot]
- GIVEN an unwatched HTML report
- WHEN the shell renders it
- THEN the document MUST carry no reload instruction and MUST state exactly one admitted revision.

### Requirement: Live report non-claims
r[kiln.observe.dashboard.html.live_update.nonclaims] A live HTML report MUST NOT claim that a reader observed a newer revision, and MUST NOT claim CI success, provider correctness, source trust, journal or store completeness, release readiness, or that the observed revision is the newest one.

#### Scenario: Live report evidence stays scoped
r[kiln.observe.dashboard.html.live_update.nonclaims.scenario.scope]
- GIVEN a rewritten live report
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the admitted revision it was rendered from, the profile, and the observed interval.
