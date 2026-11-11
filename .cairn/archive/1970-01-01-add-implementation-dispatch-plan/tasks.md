## Phase 1: Plan admission

- [x] [serial] Add the versioned plan binding work, attempt, base, workspace, provider, paths, effects, controls and budgets. r[kiln.implementation.dispatch_plan]
- [x] [serial] Refuse mismatched bindings, broader permissions, network/credentials and excessive budgets. r[kiln.implementation.dispatch_plan.scenario.refused]

## Phase 2: Reservation integration

- [x] [serial] Derive the canonical plan identity and reserve it through the attempt ledger. r[kiln.implementation.dispatch_plan.scenario.admit]
- [x] [serial] Add `reserve-plan` to the operator CLI and document it. r[kiln.implementation.dispatch_plan]
