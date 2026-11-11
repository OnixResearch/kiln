# Implementation Work Specification

## ADDED Requirements

### Requirement: Operator claim and cancel commands

r[kiln.implementation.operator_claim] The operator command MUST allow claiming one
current attempt with a positive lease and printing its canonical attempt JSON,
and cancelling that attempt with its exact identity and fence. A stale or missing
fence MUST fail. The commands MUST NOT dispatch, verify or publish, and a claim
MUST remain an explicit handoff to an admitted execution route.

#### Scenario: Claim and cancel round trip

r[kiln.implementation.operator_claim.scenario.roundtrip]
- GIVEN admitted eligible work
- WHEN the operator claims it and then cancels with the returned fence
- THEN the attempt MUST be persisted with claimed then cancelled state
- AND no active attempt MUST remain

#### Scenario: Stale fence cannot cancel

r[kiln.implementation.operator_claim.scenario.stale]
- GIVEN a cancelled or replaced attempt
- WHEN the operator cancels with its old fence
- THEN the command MUST fail
- AND the current attempt MUST remain unchanged
