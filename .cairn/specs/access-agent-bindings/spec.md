# Access Agent Bindings Specification

## Purpose

Defines the `access-agent-bindings` capability.

## Requirements

### Requirement: Bounded viewer agent declarations

r[kiln.access.agent.declaration] A viewer or tailnet binding MAY declare
`agent = { harness, model? }`. Harness and optional model MUST be non-empty,
trimmed, non-control text bounded to 128 bytes. Invalid declarations MUST reject
the access policy. Existing viewers without an agent declaration MUST remain
unchanged and serialize without the field.

#### Scenario: Invalid agent text
r[kiln.access.agent.declaration.scenario.invalid]
- GIVEN a viewer agent with empty, untrimmed, control-containing or over-bound text
- WHEN the access profile is admitted
- THEN it MUST be rejected.

### Requirement: Exact review agent derivation

r[kiln.access.agent.derivation] Access MUST expose the exact `viewer:<id>` or
`tailscale:<user-id>` principal for every viewer or tailnet binding with an agent
declaration. When code review is
configured, the service MUST union those principals into the review agent list
without duplicates before review profile admission. Review admission MUST still
require every agent to be an admitted review write principal. The declaration
MUST grant no review, execution or merge authority by itself.

#### Scenario: Agent viewer becomes a review agent
r[kiln.access.agent.derivation.scenario.merge]
- GIVEN an admitted viewer with an agent declaration and a matching review write principal
- WHEN the review workspace starts
- THEN that exact principal is treated as an agent by vote policy and submission attestation.

#### Scenario: Non-writer agent fails startup
r[kiln.access.agent.derivation.scenario.non_writer]
- GIVEN an access agent principal absent from review write principals
- WHEN the review workspace starts
- THEN startup MUST fail rather than ignore the declaration.
