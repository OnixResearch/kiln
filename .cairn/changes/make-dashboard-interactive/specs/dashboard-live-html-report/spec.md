# Dashboard live HTML changes

## MODIFIED Requirements

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
