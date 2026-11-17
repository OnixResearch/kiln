# Source projection delta

## ADDED Requirements

### Requirement: Materialized projection inputs and attempt overlays

r[kiln.source.projection.materialization] A materializer MUST accept only a completeness witness and a versioned materializer profile, and MUST derive a reusable input identity from the manifest identity, its content identity and the profile, and an attempt overlay identity from the input identity, the attempt and a non-zero fence. It MUST write only the planned entries and modes after verifying every provider object against its native identity, MUST refuse entries the profile cannot represent, MUST publish an input only after it is complete, and MUST NOT reuse or overwrite a directory whose binding is missing or mismatched. Attempt overlays MUST be private writable copies whose writes cannot change the shared input or another overlay. Materialization MUST NOT grant read, write or execution authority.

#### Scenario: Repeated materialization is equivalent

r[kiln.source.projection.materialization.scenario.repeat]
- GIVEN one complete projection and one materializer profile
- WHEN it is materialized twice
- THEN both inputs MUST have the same identity and byte- and mode-equivalent trees

#### Scenario: Overlays are isolated

r[kiln.source.projection.materialization.scenario.isolated]
- GIVEN two attempt overlays prepared from one input
- WHEN one overlay is edited
- THEN neither the input nor the other overlay MUST change

#### Scenario: Damaged or unbound state is never admitted

r[kiln.source.projection.materialization.scenario.refused]
- GIVEN a missing, corrupt or truncated object, an unsupported entry, a damaged reused input or an existing directory without a matching binding
- WHEN preparation runs
- THEN it MUST fail without publishing or overwriting any input or overlay
