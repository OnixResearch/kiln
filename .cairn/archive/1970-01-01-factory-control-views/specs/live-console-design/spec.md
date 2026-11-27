## ADDED Requirements

### Requirement: Source-bound factory control views

r[kiln.web.factory_control_views]
The console MUST display triage, change lineage, recovery, delivery, workflow and flow observations without inferring missing identities, current state, effect or authority. Actions MUST delegate to their existing owner.

#### Scenario: An owner is unavailable
- GIVEN a console view reads more than one owner
- WHEN one owner's observation is unavailable
- THEN remaining observations stay visible and unavailable data does not read as success or absence
