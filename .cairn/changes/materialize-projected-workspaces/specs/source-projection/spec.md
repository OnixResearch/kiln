# Source projection delta

## ADDED Requirements

### Requirement: Materialized projection inputs and attempt overlays

r[kiln.source.projection.materialization] A materializer MUST accept only a completeness witness and a versioned materializer profile, and MUST derive a reusable input identity from the manifest identity, its content identity and the profile, and an attempt overlay identity from the input identity, the attempt and a non-zero fence. It MUST write only the planned entries and modes after verifying every provider object against its native identity, MUST refuse entries the profile cannot represent, MUST publish an input only after it is complete, and MUST NOT reuse or overwrite a directory whose binding is missing or mismatched. Attempt overlays MUST be private writable copies whose writes cannot change the shared input or another overlay. A dispatch plan that uses a projected workspace MUST pin the admitted repository, exact source revision, projection definition, resolved manifest, materializer profile, input and overlay generation for its attempt and fence, and the retained overlay binding MUST record the same pin. Materialization MUST NOT grant read, write or execution authority.

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

#### Scenario: Dispatch and retained overlays pin the workspace

r[kiln.source.projection.materialization.scenario.pinned]
- GIVEN a dispatch plan whose workspace pins one materialization plan for its attempt and fence
- WHEN the plan is admitted, or the overlay is prepared or resumed for that pin
- THEN a pin for another attempt, fence or base revision, an overlay that is not the identity of its input, attempt and fence, a workspace identity other than that overlay, or a pin that differs from the current materialization plan or the retained overlay binding MUST be refused, and a plan without a pin MUST keep its previous identity

#### Scenario: A shared corpus has a frozen local baseline

r[kiln.source.projection.materialization.scenario.baseline]
- GIVEN a typed shared corpus with two projects, a shared dependency, an executable file, equal-byte files at two origins and an excluded entry
- WHEN every projection is materialized through the eager local materializer for two attempts
- THEN the receipt MUST record each contract check's outcome and the timings, and match the committed expectation apart from timings
