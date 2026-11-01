# Agent Audit Attribution Specification

## Purpose

Defines optional declared agent identity on attributable audit records.

## Requirements

### Requirement: Audit records carry declared agent identity

r[kiln.access.agent.audit] An audit record for an authenticated principal with a
declared agent MUST include that declaration and MUST omit it for non-agent or
anonymous principals. The field MUST be additive and MUST NOT change access,
review, execution or merge authority. Existing records without it MUST remain
valid and serialize unchanged.

#### Scenario: Agent denial is attributable
r[kiln.access.agent.audit.scenario.denial]
- GIVEN a denied request from an authenticated agent principal
- WHEN the activity audit is read
- THEN the record names the principal and its declared harness/model.

#### Scenario: Non-agent audit remains unchanged
r[kiln.access.agent.audit.scenario.human]
- GIVEN an operator, worker or undeclared viewer request
- WHEN its audit record is written
- THEN no agent field is present.
