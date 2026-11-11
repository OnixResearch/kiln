# Implementation Work Specification

## ADDED Requirements

### Requirement: Operator drain and status commands

r[kiln.implementation.operator_cli] The implementation shell MUST provide an
operator command that drains a configured authenticated delivery queue into the
durable work ledger and a command that reports retained work and attempt counts.
The drain command MUST require absolute database, queue, secret, policy and
enrollment inputs and a positive batch bound; the status command MUST read only.
Neither command MAY claim, dispatch, verify, publish or grant authority.

#### Scenario: Drain queued work

r[kiln.implementation.operator_cli.scenario.drain]
- GIVEN a configured queue containing one authenticated issue delivery
- WHEN the operator runs drain
- THEN exactly one work record MUST be admitted
- AND the command MUST report the drained count

#### Scenario: Report durable status

r[kiln.implementation.operator_cli.scenario.status]
- GIVEN an admitted work record and no attempts
- WHEN the operator runs status
- THEN it MUST report the retained work and attempt counts
- AND it MUST NOT claim or mutate work
