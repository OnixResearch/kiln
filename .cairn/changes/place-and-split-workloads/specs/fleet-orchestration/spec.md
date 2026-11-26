## ADDED Requirements

### Requirement: Explicit workload placement

r[kiln.sdlc.workload_placement]
Kiln MUST move eligible workloads and split independent units across selected hosts. Keep placement with the owner. Use exact current attempt, checkpoint, destination admission, and fencing for a move, never duplicate an unresolved execution. Split into non-overlapping units with explicit dependencies and destination admission. Distinguish plan, requested effect, and observed placement.

#### Scenario: Operator uses explicit workload placement

r[kiln.sdlc.workload_placement.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they move an eligible attempt or place two independent split units on two enrolled hosts
- THEN Kiln MUST record owner-confirmed placement and lineage; refuse overlap, cycles, stale fences and unavailable destinations
