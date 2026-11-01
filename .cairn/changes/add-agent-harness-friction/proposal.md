## Why

Review friction now counts attested agent-submitted changes, but it does not
show which declared harnesses contributed. Operators need that breakdown to
interpret agent participation without inspecting each revision.

## What Changes

- Record the declared harnesses of attested revisions on each review-window
  change.
- Aggregate a top-level `agent_harnesses` map in the review-friction view,
  counting each change once per declared harness.
- Include the map in the observation identity only when non-empty.
- Show the breakdown in dashboard plain and HTML reports.
- Add observation and dashboard tests.
