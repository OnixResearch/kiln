## ADDED Requirements

### Requirement: Typed issues and relations

r[kiln.work.issues]
The workspace MUST create and edit COB-backed issues with explicit owners, status, priority, labels, parent/sub-issue and blocking relations. Invalid identities, cycles, unsupported fields and stale writes MUST be rejected without changing admitted state.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Outcome planning

r[kiln.work.planning]
The workspace MUST persist outcome projects, teams, milestones, time-boxed cycles and initiatives as work COBs, link issues to them and expose progress and dated roadmap views without equating completion with CI or release acceptance.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Collaborative discussion

r[kiln.work.discussion]
Authenticated admitted users MUST persist editable descriptions, comments, mentions, reactions and team/project updates through native COB operations, with attributable authors and bounded inert text. Unauthorized edits MUST be rejected.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Explicit intake and triage

r[kiln.work.triage]
The workspace MUST capture internal/customer/support requests with original source links, provide triage accept/decline/snooze/duplicate/routing actions and preserve the recorded intake rather than fabricate integrations.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Principal-owned attention

r[kiln.work.personal]
The workspace MUST persist principal-owned subscriptions, read markers, favorites and reminders as COBs and derive personal assignments, mentions and watched updates. Private records MUST NOT be disclosed or mutated by other principals; notifications MUST identify bounded observation scope.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Durable views

r[kiln.work.views]
The workspace MUST persist named private/shared filtered views with sorting, grouping and board/list layouts. Favorite navigation and restored views MUST use those COB identities rather than browser-local canonical state.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Efficient interaction

r[kiln.work.interaction]
The live workspace MUST offer authenticated cross-entity command/search, keyboard navigation, quick previews, contextual actions and multiselect with explicit per-object bulk outcomes. Session clear MUST erase private results and pending requests; shortcuts MUST respect editable controls and focus.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Joined change detail

r[kiln.work.change]
One work/change detail MUST join explicitly linked source revision, patch/review, CI runs, tests, artifacts and lifecycle evidence using exact admitted identities. Missing or inaccessible facts MUST remain unavailable; recorded claims MUST NOT become lifecycle acceptance.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state

### Requirement: Native bounded write authority

r[kiln.work.authority]
An explicit optional work policy MUST bind local store, service signing key and allowed principals. Work history MUST use signed native Radicle COBs behind the pinned shared Git store port, typed deterministic reduction, bounded verified reads, exact-head writes and request replay binding. Unknown types, malformed/signature-invalid history, mismatched replay, stale/concurrent heads and unavailable signers MUST fail closed. Existing read-only COB and CI profiles MUST retain their contracts. No publish, sync, remote fetch or operator-key discovery is permitted.

#### Scenario: Admitted operation

- GIVEN an explicitly configured disposable work store and an admitted principal
- WHEN the principal performs the described workflow
- THEN the persisted signed COB history and visible projection MUST agree

#### Scenario: Rejected operation

- GIVEN an unauthorized, stale, malformed or unavailable input
- WHEN the described operation is requested
- THEN it MUST expose a bounded explicit failure without inventing success or changing unrelated state
