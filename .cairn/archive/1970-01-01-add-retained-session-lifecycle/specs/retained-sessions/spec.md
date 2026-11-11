# Retained Sessions Delta Specification

## ADDED Requirements

### Requirement: Retained session ownership and lifecycle

r[kiln.hosted.retained.lifecycle] A retained session MUST bind its logical
session identity, work/version, attempt/fence, placement host and monotonic
generation, and bounded resource reservation. Every lifecycle command MUST carry
the exact current owner session, fence, host and generation; stale owners MUST be
refused. Restore MUST observe confinement before workload effects and MUST fail
into reconciliation when confinement is absent. Migration MUST start only from a
paused source, advance the target generation on another host, and treat
migrated-away as separate from target placement, cleanup and capacity release.
Retirement MUST require an observed command and MUST be the only transition that
releases capacity. Reconciliation-required MUST retain capacity.

#### Scenario: Restore and resume under the current owner

r[kiln.hosted.retained.lifecycle.scenario.restore]
- GIVEN a receiving session owned by the current placement
- WHEN restore and positive confinement are observed under the owner
- THEN the session MUST pause and only the same owner may resume it

#### Scenario: Confinement failure

r[kiln.hosted.retained.lifecycle.scenario.confinement]
- GIVEN restore without observed confinement
- WHEN the session is observed
- THEN it MUST require reconciliation and MUST NOT run workload effects

#### Scenario: Stale owner or generation

r[kiln.hosted.retained.lifecycle.scenario.stale]
- GIVEN a stale fence, host or placement generation
- WHEN any lifecycle command runs
- THEN it MUST fail and leave the current state unchanged

#### Scenario: Migration and source retirement

r[kiln.hosted.retained.lifecycle.scenario.migration]
- GIVEN a paused source and a higher target generation on another host
- WHEN migration starts and the source is observed migrated away
- THEN the source MUST NOT resume
- AND capacity MUST remain occupied until retirement is observed
