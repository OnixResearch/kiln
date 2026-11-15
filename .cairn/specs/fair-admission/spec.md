# Fair Admission Specification

## Purpose

Defines the `fair-admission` capability.

## Requirements

### Requirement: Bounded opt-in admission
r[kiln.admission.bounded]
Kiln MUST filter existing work eligibility before existing fairness ranking, bound ready queues and lineage budgets, and preserve exact owner operation/generation debits and uncertainty. Transfers MUST NOT count as consumption. Shared deployment allocations MUST be charged once while worker requests retain separate charges.

#### Scenario: Competing child burst
- GIVEN two comparable campaigns and an owner-confirmed reservation
- WHEN one campaign queues children
- THEN immediate held debits and aggregate parent ceilings MUST prevent quota multiplication

### Requirement: Durable effect gating
r[kiln.admission.durable]
Explicit opt-in admission MUST revalidate current work, claim/fence, policy, deadline and provider generation after grant and persist before dispatch. Unknown effects MUST retain accounting across restart; waiting MUST NOT reset consumed attempts or authorize fallback.

#### Scenario: Possible acceptance
- GIVEN a persisted dispatch intent and lost response
- WHEN the store reopens
- THEN reconciliation MUST retain the debit and MUST NOT blindly repeat dispatch or falsely refund it
