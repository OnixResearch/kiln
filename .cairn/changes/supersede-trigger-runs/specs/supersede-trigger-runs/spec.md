# Supersede Queued Trigger Runs Specification

## Purpose

Defines opt-in cancellation of stale active and queued runs when a new trigger
dispatch succeeds for the same project and requested reference.

## Requirements

### Requirement: Opt-in trigger policy

r[kiln.web.supersede.policy] A trigger MUST opt into queued supersession with
`supersede_queued = true`; absence or false MUST keep the previous behavior.
The Nickel automation contract MUST accept the optional field.

#### Scenario: Policy is off
r[kiln.web.supersede.policy.scenario.off]
- GIVEN a trigger without `supersede_queued`
- WHEN a new push dispatches
- THEN older queued runs MUST remain unchanged.

#### Scenario: Policy is on
r[kiln.web.supersede.policy.scenario.on]
- GIVEN a trigger with `supersede_queued = true`
- WHEN three pushes for the same project/reference leave one active and two queued
- THEN the older queued run MUST become Cancelled and the newest run MUST remain queued.

### Requirement: Exact identity match

r[kiln.web.supersede.identity] Supersession MUST match only runs with the same
project and requested symbolic reference and a different exact revision. Runs
without a recorded reference MUST NOT match.

#### Scenario: Other references are untouched
r[kiln.web.supersede.identity.scenario.reference]
- GIVEN a queued run for another project or requested reference
- WHEN a trigger dispatches
- THEN that run MUST NOT be cancelled.

### Requirement: Queued and active cancellation

r[kiln.web.supersede.cancel] Supersession MUST complete older queued runs as
Cancelled through the durable request-replay path. Failure to cancel an older
run MUST NOT fail the new dispatch.

r[kiln.web.supersede.active] Supersession MUST cancel older active runs through
the exact admitted provider control path, using the run's admitted attempt,
revision, sequence and execution. Provider refusal or stale control MUST be
best-effort. Awaiting-approval and terminal runs MUST NOT be cancelled.

#### Scenario: Active run is cancelled
r[kiln.web.supersede.active.scenario.running]
- GIVEN an active running run for the same project/reference
- WHEN a newer push dispatches
- THEN the running run MUST receive exactly one provider cancellation and complete as Cancelled.

#### Scenario: Awaiting approval is untouched
r[kiln.web.supersede.active.scenario.awaiting]
- GIVEN an awaiting-approval run
- WHEN a newer push dispatches
- THEN the awaiting-approval run MUST NOT be cancelled.
