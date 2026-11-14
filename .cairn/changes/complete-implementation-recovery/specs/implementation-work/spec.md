# Implementation Recovery Specification Delta

## ADDED Requirements

### Requirement: Current durable result authority

r[kiln.implementation.current_authority] The durable shell MUST admit worker results and dispatch plans only for a retained exact work identity and current attempt/fence. A cancelled, superseded or replaced attempt MUST NOT regain authority through an unexpired lease. Source supersession MUST atomically apply its configured abort or finish rule while preserving the frozen attempt contract.

#### Scenario: Cancelled worker returns before lease expiry

r[kiln.implementation.current_authority.scenario.cancelled]
- GIVEN a cancelled attempt whose lease time has not elapsed
- WHEN its worker submits a handoff
- THEN the result MUST be refused without changing retained evidence

### Requirement: Unknown execution reconciliation barrier

r[kiln.implementation.unknown_barrier] A dispatch with an unknown provider outcome MUST remain distinguishable from implementation failure. A replacement attempt MUST NOT be admitted until the possible execution has been reconciled; cancellation and lease expiry MUST NOT establish provider death or refund budget. Retained recovery observations MUST survive restart.

#### Scenario: Lost acknowledgement and restart

r[kiln.implementation.unknown_barrier.scenario.restart]
- GIVEN a reserved dispatch and an unknown acknowledgement
- WHEN the ledger restarts and a scheduler tries to claim the same work
- THEN the scheduler MUST be refused pending explicit reconciliation
- AND independent work MAY remain eligible

### Requirement: Bounded recovery operator consumer

r[kiln.implementation.recovery_cli] The existing operator shell MUST expose bounded exact attempt recovery and supersession transitions. An empty ready set MUST NOT establish project completion. Provider execution, independent verification, publication, integration and source reconciliation MUST remain separate outcomes.

#### Scenario: Recovery preserves distinct outcomes

r[kiln.implementation.recovery_cli.scenario.distinct]
- GIVEN retained implementation and provider observations
- WHEN the operator requests recovery
- THEN the output MUST name attempts needing reconciliation without redispatching or declaring completion
