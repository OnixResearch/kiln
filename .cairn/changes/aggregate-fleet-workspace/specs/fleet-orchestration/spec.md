## ADDED Requirements

### Requirement: Machine-first Fleet observations

r[kiln.fleet.aggregate_workspace]
The console MUST distinguish enrolled machines from observed Herdr sessions and exact Fleet assignments, retain independent Cairn and issue sources, and NOT infer a healthy or idle host or matched source when the evidence is missing.

#### Scenario: Peer enrollment is not telemetry
- GIVEN a roster with an observed machine, an unreachable target and a peer without a Herdr route
- WHEN the operator opens Fleet
- THEN all three appear with distinct observation states and the work view preserves independently observed Cairn and issue identities
