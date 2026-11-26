## ADDED Requirements

### Requirement: Queued workload rebind

r[kiln.sdlc.queued_workload_move]
Kiln MUST rebind an exact current unstarted interactive reservation to an explicitly selected admitted host in the native placement owner. It MUST compare the reservation and fence atomically against session binding and transport start, preserve the attempt and work, and retain replayable intent separately from provider observation. Kiln MUST NOT treat quota failover or active migration as a queued rebind.

#### Scenario: Operator moves a queued workload

r[kiln.sdlc.queued_workload_move.scenario.operator]
- GIVEN exact queued work and an enrolled reachable destination with compatible capacity and a repository checkout
- WHEN an operator previews and submits an unchanged move
- THEN only the selected destination may launch; stale, unavailable and potentially started work is refused, and replay or ambiguous observation never creates another execution
