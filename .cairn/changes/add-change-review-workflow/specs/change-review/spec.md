## ADDED Requirements

### Requirement: Change-centric review workflow

r[kiln.review.workflow]
Kiln MUST retain stable change identities across exact revision history, owner/WIP state, reviewer/CC requests, revision-bound votes, attention, dependency stacks, private saved queries and durable review notifications. It MUST support exact anchored threads, replies, resolution, private drafts and atomic publication. New revisions MUST invalidate old approval authority and file markers. Search MUST support awaiting review, owned changes needing revision and unresolved discussions.

#### Scenario: Review publication and revision update

- GIVEN a change with a requested eligible reviewer and exact source revision
- WHEN that reviewer drafts comments and a vote, publishes them together, and the owner uploads a new revision
- THEN other principals MUST not see unpublished drafts, publication MUST be atomic, history and notifications MUST remain durable, and the prior vote MUST not approve the new revision

### Requirement: Exact review providers

r[kiln.review.provider]
Versioned provider ports MUST support capability admission, exact revision comparison, submission and author maintenance operations. Provider shells MUST recheck exact source and target before effects, reserve intended operations durably before dispatch, reconcile uncertain outcomes and reject unsupported operations before effects. Read-only source configuration MUST NOT grant mutation authority.

#### Scenario: Moved target and replay

- GIVEN an admitted candidate and expected target
- WHEN the target moves, or an operation request is replayed after an ambiguous result
- THEN submission MUST fail closed on stale evidence and reconciliation MUST distinguish applied, not applied and uncertain outcomes without blindly repeating effects

### Requirement: Authenticated review authority

r[kiln.review.authority]
Review persistence and APIs MUST bind authenticated principals, expected state and idempotency identities. Submit requirements MUST explain eligibility, non-author approval, vetoes, WIP, exact candidate CI, unresolved discussions and dependency readiness; CI facts MUST come from canonical server state, never caller assertions. Typed configuration MUST explicitly admit writable providers and eligible submitters.

#### Scenario: Forged approval and stale CI

- GIVEN an owner, an ineligible voter, or successful CI for another source revision
- WHEN approval or submission is requested
- THEN the operation MUST NOT grant merge permission and the checklist MUST identify the unmet requirement

### Requirement: Interactive review surface

r[kiln.review.surface]
The console MUST offer change pages with revision selection/history, changed-file navigation, unified and side-by-side comparisons, revision-bound reviewed-file markers, line permalinks, anchored discussions, reviewer requests, votes, submit checklists, stacks, maintenance actions, review search, saved views and notifications. Consequential provider actions MUST confirm exact operation, target and revision values. Existing work and retained receipt routes MUST remain available.

#### Scenario: Desktop and mobile review

- GIVEN an authenticated operator and a configured temporary review repository
- WHEN the operator reviews a diff, publishes comments and votes, inspects blocked requirements and performs an authorized action
- THEN the actual page MUST remain usable by keyboard at desktop/mobile sizes, display exact identities and real results, and never present CI/Patchgate evidence as merge authority
