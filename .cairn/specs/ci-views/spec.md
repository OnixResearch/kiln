# Ci Views Specification

## Purpose

Defines the `ci-views` capability.

## Requirements

### Requirement: Failure-first run inspection

r[kiln.web.failure_first_run]
When the URL names no node or stream, a failed run MUST open on the first failed node's retained output, preferring stderr. Lines matching a bounded error pattern MUST be highlighted without altering the retained text, and a control MUST move focus to the first such line. A node or stream named in the URL MUST be honoured. Rerun and Cancel MUST be available from the run header when the run's controls allow them, through the existing exact-identity confirmation.

#### Scenario: Open a failed run

r[kiln.web.failure_first_run.scenario.failed]
- GIVEN a retained failed run whose failing node wrote to stderr
- WHEN the operator opens it without node or stream parameters
- THEN the failing node's stderr MUST be shown with error lines highlighted, and the first error MUST be reachable with one action

### Requirement: Project history page

r[kiln.web.project_page]
Each enrolled project MUST have a page derived from retained runs. The page MUST show its current outcome streak, recent runs linking to exact executions, and its last success and last failure. It MUST also show a duration trend that uses only runs with recorded timing and states how many runs were excluded.

#### Scenario: Untimed runs excluded

r[kiln.web.project_page.scenario.untimed]
- GIVEN a project with timed and untimed retained runs
- WHEN its page renders
- THEN the trend MUST plot only timed runs and MUST state the number excluded

### Requirement: Retained runs table

r[kiln.web.runs_table]
Retained runs MUST be browsable as a table grouped by local day. The table MUST show status, project, revision, duration and start time columns. Sorting MUST apply to the loaded page and say so. Existing server-side filters, cursor paging and URL-restored selection MUST be preserved.

### Requirement: Live run progress

r[kiln.web.live_progress]
An active run MUST show elapsed time since its recorded start and node completion progress. These MUST update from the existing live stream, and timers MUST stop on terminal state, route change or session clearing. Logs MUST follow an active run unless the operator scrolls away or disables following.
