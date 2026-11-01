# Agent Friction Observability Specification

## Purpose

Defines attested-agent contribution counts in the review-friction observation.

## Requirements

### Requirement: Agent-submitted change counts

r[kiln.observe.review.agent] A review window change MUST record whether any
admitted revision carries an attested agent submission. The friction view MUST
count those changes per declared authorship class as
`agent_submitted_changes`. False/zero values MUST be omitted from serialization,
and zero identities MUST remain identical to earlier encodings. The dashboard
review-friction table MUST display the count.

#### Scenario: Attested agent revision is counted
r[kiln.observe.review.agent.scenario.count]
- GIVEN an in-window change with one attested agent revision
- WHEN the review-friction view is projected
- THEN its authorship class count includes that change once.

#### Scenario: Unattested change is not counted
r[kiln.observe.review.agent.scenario.unattested]
- GIVEN changes without attested revisions
- WHEN the view is projected
- THEN `agent_submitted_changes` is zero/omitted and the earlier identity is unchanged.

### Requirement: Observation remains non-authoritative

r[kiln.observe.review.agent.boundary] The count MUST NOT alter authorship basis,
review authority, CI requirements, merge readiness or release eligibility. It
MUST distinguish declared principal attestation from verified execution.

#### Scenario: No authority change
r[kiln.observe.review.agent.boundary.scenario.authority]
- GIVEN a friction view with agent-submitted counts
- WHEN a consumer reads it
- THEN the existing non-authoritative basis and non-claims remain unchanged.
