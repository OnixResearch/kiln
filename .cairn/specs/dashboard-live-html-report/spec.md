# Dashboard Live Html Report Specification

## Purpose

Defines the `dashboard-live-html-report` capability.

## Requirements

### Requirement: Live HTML dashboard report
r[kiln.observe.dashboard.html.live_update] A watched HTML report MUST be rewritten on the profile's watch interval, MUST ask the reader to reload at that interval without carrying a script, and MUST replace its file atomically so a reader never observes a partial document.

#### Scenario: A watched report is rewritten and asks for a reload
r[kiln.observe.dashboard.html.live_update.scenario.match]
- GIVEN a watched HTML report and an admitted dashboard profile
- WHEN the shell renders a frame
- THEN it MUST replace the report file atomically, and the document MUST carry a reload instruction at the profile's watch interval and MUST carry no script.

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
