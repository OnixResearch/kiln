## Phase 1: Execution identity

- [x] [serial] Refuse an acknowledged or reconciled run already recorded on another attempt, after fence and phase authorization. r[kiln.implementation.execution_identity]

## Phase 2: Race fixtures

- [x] [serial] Add pure ledger fixtures for a late handoff after cancellation, predecessor callbacks after replacement and callbacks for absent attempts, restoring the serialized ledger between callbacks. r[kiln.implementation.execution_identity.scenario.crossed] r[kiln.implementation.execution_identity.scenario.late]

## Phase 3: Verification

- [x] [serial] Record the race coverage table in the implementation runbook, update the changelog, register the delta root, and pass focused tests, clippy and Cairn gates. r[kiln.implementation.execution_identity]
