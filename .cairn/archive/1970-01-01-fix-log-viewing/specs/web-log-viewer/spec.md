# Web log viewer

## ADDED Requirements

### Requirement: Preserve admitted log continuation
r[kiln.dashboard.logs.continuation]

For one exact run, node, stream and search, the log viewer MUST append reads from its admitted next cursor without discarding previous displayed text, including empty reads. An explicit seek MUST replace the displayed range only after an admitted response. Failed reads MUST preserve previously admitted text with an explicit unavailable/stale status. Selection changes MUST invalidate obsolete responses. Display retention MUST remain bounded and explicit.

#### Scenario: Contiguous chunks and end cursor
- GIVEN retained output spanning multiple bounded reads
- WHEN the operator continues reading through the current end cursor
- THEN the viewer preserves the concatenated admitted chunks without duplication or disappearance.

#### Scenario: Search and native diagnostics
- GIVEN a literal worker-stream search
- WHEN the operator changes the search or selects native diagnostics
- THEN the viewer loads the changed search and never submits unsupported search fields to diagnostics.

#### Scenario: Inspecting older followed text
- GIVEN following is active and the operator scrolls away from the tail
- WHEN a new chunk or empty read arrives
- THEN the viewer preserves the inspected scroll position while retaining existing cursor and display bounds.
