# Implementation Work Specification

## ADDED Requirements

### Requirement: Durable atomic implementation state

r[kiln.implementation.durable_store] The implementer shell MUST persist the work
and attempt ledgers in a durable store and mutate them inside one immediate
transaction, so two schedulers cannot both claim one work unit. Every mutation
MUST be visible after restart, and a failed operation MUST leave the prior state
unchanged. The shell MUST NOT execute a worker, verify a candidate, publish, or
grant authority.

#### Scenario: Concurrent schedulers

r[kiln.implementation.durable_store.scenario.concurrent]
- GIVEN two shell instances over one store and one eligible work unit
- WHEN both attempt a claim
- THEN exactly one claim MUST succeed
- AND the other MUST observe no available claim

#### Scenario: Restart preserves dedup and budget

r[kiln.implementation.durable_store.scenario.restart]
- GIVEN admitted work, attempts and consumed budget
- WHEN the store is closed and reopened
- THEN duplicate deliveries MUST still deduplicate
- AND consumed budget MUST still bound new claims

### Requirement: Recovery reports expiry and reconciliation set

r[kiln.implementation.recovery] Recovery MUST expire ended leases, revoke their
fences, and report the active attempts that still require provider
reconciliation. It MUST NOT redispatch or invent an execution.

#### Scenario: Recovery after restart

r[kiln.implementation.recovery.scenario.restart]
- GIVEN an attempt whose lease ended while the process was stopped
- WHEN recovery runs after restart
- THEN the attempt MUST be reported expired and blocked
- AND an unacknowledged dispatch MUST be reconcilable without a second run
