# Implementation Work Specification

## ADDED Requirements

### Requirement: Cairn implementation-ready snapshot intake

r[kiln.implementation.cairn_intake] Kiln MUST admit implementation work from a
`cairn.implementation-ready.v1` snapshot only when the snapshot is complete and
its schema is supported. A ready selection MUST become one work record per stable
`task:<id>` or legacy `change:<name>` unit, bind the change snapshot digest as
the source version, carry the unit's requirement references as explicit
acceptance obligations, and use a caller-supplied monotonic sequence. Non-ready
outcomes MUST produce no work. An incomplete enumeration, incomplete snapshot,
forged or inconsistent selected unit, or already archive-ready selection MUST
fail. The intake MUST NOT claim, execute, verify or mutate Cairn lifecycle state.

#### Scenario: Ready unit becomes work

r[kiln.implementation.cairn_intake.scenario.ready]
- GIVEN a complete ready snapshot with one selected task unit
- WHEN intake runs at a monotonic sequence
- THEN one work record MUST bind the change/unit alias, digest and requirement references
- AND its acceptance MUST include both operator policy and unit requirements

#### Scenario: Non-ready outcomes produce no work

r[kiln.implementation.cairn_intake.scenario.outcome]
- GIVEN blocked, empty or completion-only Cairn outcomes
- WHEN intake runs
- THEN no work MUST be created

#### Scenario: Incomplete or forged snapshot fails

r[kiln.implementation.cairn_intake.scenario.refused]
- GIVEN an incomplete snapshot, `incomplete_limit`, a mismatched digest, an archive-ready selection or inconsistent gates
- WHEN intake runs
- THEN it MUST fail before retaining work

#### Scenario: Invalid intake policy fails

r[kiln.implementation.cairn_intake.scenario.policy]
- GIVEN an escaping path, empty acceptance, invalid budget or malformed identity
- WHEN intake runs
- THEN it MUST fail before reading the snapshot into work
