# Implementation Work Specification

## ADDED Requirements

### Requirement: Queue drain admits each delivery once

r[kiln.implementation.queue_drain] A scheduler consumer MUST drain bounded
authenticated issue deliveries into the implementer ledger by re-verifying the
signature and re-normalizing through operator policy and the versioned work
contract. A delivery MUST be removed only after admission or an explicit closed
or ignored decision. A forged or unverifiable delivery MUST fail without being
removed, so it cannot be silently lost. Repeated delivery MUST resolve to the
existing work version rather than a new one.

#### Scenario: Drain once

r[kiln.implementation.queue_drain.scenario.once]
- GIVEN one queued authenticated issue delivery
- WHEN the consumer drains it twice
- THEN exactly one work version MUST exist
- AND the queue MUST be empty after each successful drain

#### Scenario: Forged queued record

r[kiln.implementation.queue_drain.scenario.forged]
- GIVEN a queued record whose signature does not verify
- WHEN the consumer drains the queue
- THEN it MUST fail
- AND the record MUST remain queued
