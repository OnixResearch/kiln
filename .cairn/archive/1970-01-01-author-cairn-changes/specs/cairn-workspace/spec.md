## ADDED Requirements

### Requirement: Cairn authoring and dependencies

r[kiln.sdlc.cairn_authoring]
Kiln MUST create and edit Cairn changes and dependency edges from Kiln. Use explicitly enrolled repository roots, fixed native Cairn verbs and admitted artifacts. Show the exact current revision, reject stale writes and cycles, and preserve the provider result without implicitly committing or starting work.

#### Scenario: Operator uses cairn authoring and dependencies

r[kiln.sdlc.cairn_authoring.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they create a change then add or remove a prerequisite
- THEN Kiln MUST read back the native change and new graph; refuse invalid edges, stale revisions and unauthorized writes
