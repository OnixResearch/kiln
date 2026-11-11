# Change Review Specification

## ADDED Requirements

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
