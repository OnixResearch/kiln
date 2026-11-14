## ADDED Requirements

### Requirement: Durable retained mobility operations
r[kiln.hosted.mobility.runtime]
The hosted shell MUST bind retained effects to exact attempt, fence, placement and protected resources, persist intent before mutation, refuse redispatch after uncertainty, and release capacity only after observed retirement. Bounded authenticated migration streams MUST remain separate from durable object verification, ownership commit and destination resume. Running migration MUST remain denied without exact runtime/hardware compatibility and qualified write/effect fencing.

#### Scenario: Repeated command and crash
r[kiln.hosted.mobility.runtime.scenario.replay]
- GIVEN an operation durably dispatched under an exact current placement
- WHEN the controller crashes or receives a repeated command
- THEN the shell MUST retain uncertain state and capacity without replaying effects or migration bytes

#### Scenario: Verified transfer and cleanup
r[kiln.hosted.mobility.runtime.scenario.durable]
- GIVEN a bounded paired checkpoint and exact authenticated peer
- WHEN all objects are verified and durably published
- THEN delivery MUST NOT authorize resume or cleanup without independent fencing, placement and retirement evidence
