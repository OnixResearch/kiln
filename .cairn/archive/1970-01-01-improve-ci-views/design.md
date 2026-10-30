## Context

The run view (`page=runs`) shows a run-list sidebar, a header card with controls inside a disclosure, and tabs, with a split pipeline/logs layout on wide screens. The log panel puts node, stream, cursor and search controls above the output. Existing APIs provide:

- `/api/runs`: bounded and filterable by project, outcome and source, with cursor paging;
- `/api/runs/{id}` and `/api/runs/{id}/logs|artifacts|reports|telemetry|...`;
- `/api/projects`;
- `/api/overview`;
- `/api/live`: an SSE stream of version changes;
- the confirmed control flow through `#control-dialog`.

Accepted contracts include the following requirements:

- `kiln.dashboard.complete.follow`, `.streaming`, `.reruns` and `.navigation`;
- `kiln.web.log_viewer`;
- `kiln.web.readable_observations`, `kiln.web.console_layout` and `kiln.web.page_consistency`;
- `kiln.web.minimal_console`, which requires a Logs link to open the exact execution without substitution.

## Goals and Success Contract

Opening a failed run shows its first error line without scrolling or choosing a node or stream. An operator can rerun or cancel from the header, through the same confirmation. A project page answers four questions: is it healthy, since when, how long does it take, and what failed last. Runs can be scanned and sorted as a table. An active run visibly progresses.

False completion includes:

- selecting a different node or run than the URL names;
- highlighting that edits the retained text;
- trend points invented for untimed runs;
- controls that skip confirmation;
- polling loops that ignore session clearing;
- checking only on desktop.

## Decisions

### Decision: Failure-first defaults that respect explicit URLs

**Choice:** When the URL names no node or stream, a failed run defaults to the first node in a failed state and to stderr, falling back to stdout when stderr is empty. When the URL names a node or stream, it wins. Error lines are recognized with a bounded, conservative pattern (error, panicked, FAILED, failed, fatal, Traceback, exit code) and wrapped in `<mark>`-style spans after inert text insertion. The retained text is never altered. "Jump to first error" scrolls to and focuses the first highlighted line. Node, stream, cursor and search controls sit in a single toolbar row, with advanced cursor controls behind a disclosure.

### Decision: Visible header actions over the existing control flow

**Choice:** Rerun (latest or exact, matching current rerun modes) and Cancel buttons appear in the run header when the run's `controls` allow them. They open the existing `#control-dialog` with the same exact facts. Pause and resume remain in the disclosure.

### Decision: Project page as a new route and asset

**Choice:** Add `page=project&project=<id>`, rendered by a separate `project.js`/`project.css` served at `/project.js` and `/project.css`. The page reads `/api/runs?project_id=<id>` (bounded pages) and `/api/projects`. It shows:

- a header with health and a "Run latest" primary action, using the existing start flow and confirmation semantics;
- the current streak;
- a timeline of recent runs as status dots linking to exact runs;
- a duration trend drawn as inline SVG over timed runs only, with the count of excluded runs stated;
- last success and last failure cards with Logs links.

Projects cards and overview rows link to it.

### Decision: Runs table

**Choice:** The run list becomes a table with columns for status, project, revision (short, exact in title), duration, started (relative) and actions. Rows are grouped by local day, and column headers sort the loaded page client-side, labelled as such. Server-side filters and cursor paging stay unchanged. On wide screens the selected run opens in a detail pane beside the table; on narrow screens it opens below. Row selection updates the URL as today.

### Decision: Live progress from the existing stream

**Choice:** While a run is active, the header shows elapsed time ticking from its recorded start, and node progress (n of m complete). Logs follow by default only for active runs the operator has not scrolled away from. Updates are driven by the existing `/api/live` versions. Timers stop on session clear, route change and terminal state. The overview marks running projects.

## Risks / Trade-offs

- Error highlighting can false-positive. It is presentation only, and the exact text stays visible.
- Client-side sorting covers only the loaded page, and the UI says so.
- One owner edits web.js; the project page lives in separate files to avoid conflicts.

## Verification Strategy

Use real-data candidate review on a failed run, a passed run, an untimed run, project pages for a failing and a passing project, and the runs table. Check widths of 320, 1440 and 1920, light and dark, and keyboard use. Exercise an active run through the maintained native smoke exercise. Run the repository gates and the node test for the error-line and duration-trend helpers. Then deploy and verify the live surface.
