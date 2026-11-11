# Implementation Work Specification

## ADDED Requirements

### Requirement: Bounded drain loop

r[kiln.implementation.drain_loop] The operator command MUST provide a repeated
drain mode with a positive interval, an optional positive iteration bound, and
shutdown on SIGTERM or SIGINT. It MUST report the completed iteration and drained
delivery counts. A drain error MUST stop the loop. The loop MUST NOT claim,
dispatch, verify or publish work.

#### Scenario: Bounded loop

r[kiln.implementation.drain_loop.scenario.bounded]
- GIVEN an empty queue and an iteration bound of two
- WHEN the operator runs the loop
- THEN it MUST complete two iterations and report zero drained deliveries
- AND it MUST exit successfully
