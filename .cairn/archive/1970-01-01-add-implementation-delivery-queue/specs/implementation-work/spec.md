# Implementation Work Specification

## ADDED Requirements

### Requirement: Durable authenticated issue-delivery queue

r[kiln.implementation.delivery_queue] When configured, the GitHub webhook shell
MUST authenticate an `issues` delivery and store its exact bytes in a bounded
durable queue for a separate implementation scheduler. Repeating a delivery
identity MUST be idempotent. A full queue MUST refuse rather than evict, and an
invalid signature or non-issue event MUST NOT be queued. Absence of the queue
configuration MUST preserve prior behavior. Storing a delivery MUST NOT admit,
claim, execute or accept work.

#### Scenario: Idempotent queueing

r[kiln.implementation.delivery_queue.scenario.idempotent]
- GIVEN a configured queue and one authenticated issue delivery
- WHEN the same delivery is stored twice
- THEN exactly one queued record MUST remain
- AND the second store MUST report a duplicate

#### Scenario: Forged delivery is not queued

r[kiln.implementation.delivery_queue.scenario.forged]
- GIVEN an invalid signature or a non-issue event
- WHEN the shell handles it
- THEN it MUST fail or ignore the event without storing a record
