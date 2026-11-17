# Implementation work delta

## ADDED Requirements

### Requirement: One provider execution per attempt

r[kiln.implementation.execution_identity] A provider workflow execution identity MUST bind at most one implementation attempt. Acknowledging or reconciling a dispatch with a run identity that another attempt already records MUST be refused without mutating either attempt, after the caller's attempt and fence are authorized.

#### Scenario: A predecessor's run cannot settle the successor

r[kiln.implementation.execution_identity.scenario.crossed]
- GIVEN a predecessor attempt that recorded a workflow run and a current replacement attempt with a reserved dispatch
- WHEN the replacement is acknowledged or reconciled with the predecessor's run
- THEN the call MUST be refused, the replacement MUST keep no recorded run and remain current, and the predecessor MUST be unchanged

#### Scenario: Late and absent callbacks never mutate the ledger

r[kiln.implementation.execution_identity.scenario.late]
- GIVEN a cancelled attempt, a revoked predecessor, or an attempt identity that was never admitted
- WHEN its acknowledgement, candidate handoff, failure or cancellation arrives, before and after the ledger is restored from its serialized form
- THEN each call MUST be refused, no attempt MUST be created, resurrected or attached to a candidate, and the attempt budget MUST be unchanged
