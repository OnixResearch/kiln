# Agent Session Identity Specification

## Purpose

Defines optional declared agent identity on authenticated access principals and
the workspace session surface.

## Requirements

### Requirement: Session carries declared agent identity

r[kiln.access.agent.session] An authenticated viewer or tailnet principal whose
binding declares an agent MUST carry that declaration. `/api/session` MUST
expose it as configuration and MUST omit the field for non-agent principals.
Exposing it MUST NOT change read/write permissions, review authority,
execution authority or merge authority.

#### Scenario: Agent session
r[kiln.access.agent.session.scenario.agent]
- GIVEN an authenticated viewer with a declared agent
- WHEN the session is read
- THEN the principal includes the declared harness/model and the same permissions as any viewer.

#### Scenario: Non-agent session
r[kiln.access.agent.session.scenario.human]
- GIVEN an authenticated operator, worker or undeclared viewer
- WHEN the session is read
- THEN no agent field is present.

### Requirement: Declared identity is visibly labelled

r[kiln.access.agent.session.surface] The workspace principal facts MUST show a
declared agent harness/model when present and MUST label it as declared rather
than verified. No authority control may be enabled by that label.

#### Scenario: Workspace label
r[kiln.access.agent.session.surface.scenario.label]
- GIVEN an admitted agent session
- WHEN the workspace opens
- THEN the principal facts show the declared harness/model and the existing
  permissions remain unchanged.
