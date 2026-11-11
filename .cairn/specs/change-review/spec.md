# Change Review Specification

## Purpose

Defines the `change-review` capability.

## Requirements

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

### Requirement: Typed check evidence

r[kiln.review.checks] CI evidence MUST expose named check results bound to the
exact reviewed commit, with a known check kind, an outcome, and an optional
artifact digest. A configured required check MUST be satisfied only by a
passing check of the required kind and optional name on the exact winning CI
run. Unknown or duplicate requirements MUST fail policy admission. Typed checks
are evidence and MUST NOT grant gate, merge or release authority by themselves.

#### Scenario: Required check passes

- GIVEN a policy requiring a `property` check named `property-suite`
- WHEN the exact winning CI run retains a passing check of that kind and name
- THEN the `checks` checklist item is satisfied.

#### Scenario: Wrong kind or commit

- GIVEN the same requirement and a check that is failing, has another kind, or
  belongs to another commit
- WHEN the checklist is evaluated
- THEN the requirement remains unmet.

#### Scenario: Invalid policy

- GIVEN an unknown kind, duplicate kind/name pair, empty name, or oversized
  requirement list
- WHEN policy admission runs
- THEN it MUST reject without changing review authority.

### Requirement: Agent thread outcomes

r[kiln.review.agent.outcomes] Review observation MUST expose an outcome for each
agent-opened thread: `open`, `addressed`, `dismissed`, or `unknown`. An
`addressed` outcome MUST require a later revision before resolution whose exact
base matches the thread anchor and whose hunk overlaps the anchored range on
the anchored side. A resolved thread without an exact available comparison MUST
remain `unknown`. Outcomes MUST be read-only and MUST NOT grant review, merge,
gate, or release authority.

#### Scenario: Anchored hunk changes

- GIVEN an agent thread anchored to a revision-side hunk and resolved on a later
  exact-base revision
- WHEN a later revision's comparison has an overlapping hunk
- THEN the outcome is `addressed`.

#### Scenario: Resolved without a change

- GIVEN the same thread resolved on a later exact-base revision with no
  overlapping hunk
- WHEN the outcome is derived
- THEN the outcome is `dismissed`.

#### Scenario: Unresolved or unavailable history

- GIVEN an unresolved thread, a legacy thread without a resolution revision, a
  rebased later revision, or an unavailable comparison
- WHEN the outcome is derived
- THEN it is `open` or `unknown`, never `addressed` by guesswork.

#### Scenario: Window observation

- GIVEN agent threads created inside a bounded half-open window
- WHEN the read-only outcome endpoint is read
- THEN per-agent addressed, dismissed, open, unknown counts and addressed rate
  are returned without changing review state.

### Requirement: Advisory agent objections

r[kiln.review.agent.objections] Under the default advisory agent-vote policy,
agent `Veto` and `NeedsRevision` votes and agent-opened blocking threads MUST NOT
block the `vetoes` or `threads` checklist items or change checklist readiness.
They MUST remain visible in a separate non-binding advisory list. Under an
explicit binding policy, they MUST block exactly as before. Agent identity MUST
come from trusted shell facts, and persisted review state and policy bytes MUST
remain unchanged.

#### Scenario: Advisory agent objects

- GIVEN a declared agent and the default advisory policy
- WHEN the agent casts a veto or opens a blocking thread
- THEN the gate items remain satisfied, readiness is unchanged, and the
  objection appears only in the advisory list.

#### Scenario: Binding policy

- GIVEN the same agent vote or blocking thread and `agent_votes = binding`
- WHEN the checklist is evaluated
- THEN the veto and thread items block submission and the advisory list is empty.

#### Scenario: Human objection remains blocking

- GIVEN a blocking thread opened by a human principal
- WHEN the checklist is evaluated
- THEN it continues to block regardless of later agent participation.
