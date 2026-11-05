# Change Review Specification

## ADDED Requirements

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
