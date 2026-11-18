# Retained Mobility Runtime Specification

## Purpose

Defines the `retained-mobility-runtime` capability.

## Requirements

### Requirement: Durable retained mobility operations
r[kiln.hosted.mobility.runtime]
The hosted shell MUST bind retained effects and control responses to exact operation ID, session, attempt, fence, placement and protected resources. Pause/resume MUST persist requested and in-progress states before effects, and MUST adopt `Paused`/`Running` only after matching native state and required confinement/backend observations. Requested, in-progress, observed-complete, rejected and uncertain outcomes MUST remain explicit; pending or uncertain controls MUST serialize conflicting commands and retain occupied capacity through restart/reconciliation. The shell MUST refuse redispatch after uncertainty and release capacity only after observed retirement. Bounded authenticated migration streams MUST remain separate from durable object verification, ownership commit and destination resume. Running migration MUST remain denied without exact runtime/hardware compatibility and qualified write/effect fencing. Aspen placement/resource authority, guest reconnect, cross-host streaming and two-host qualification remain external blockers; the shell MUST NOT emulate them.

#### Scenario: Repeated command and crash
r[kiln.hosted.mobility.runtime.scenario.replay]
- GIVEN an operation durably dispatched under an exact current placement
- WHEN the controller crashes or receives a repeated command
- THEN the shell MUST retain uncertain state and capacity without replaying effects or migration bytes

#### Scenario: Pause request is not observation
r[kiln.hosted.mobility.runtime.scenario.control]
- GIVEN a running session and an exact pause operation
- WHEN the request is persisted or accepted but native readback remains `Running`
- THEN the session MUST NOT become paused or admit checkpoint/handoff, and its operation MUST remain uncertain for reconciliation

#### Scenario: Exact control readback
r[kiln.hosted.mobility.runtime.scenario.control-observed]
- GIVEN an unfinished control bound to the current operation and owner fence
- WHEN the exact operation receives a native `Paused` or `Running` readback plus required confinement/backend barriers
- THEN only that target state MUST be adopted; crossed or late replies MUST NOT affect another operation or successor owner

#### Scenario: Verified transfer and cleanup
r[kiln.hosted.mobility.runtime.scenario.durable]
- GIVEN a bounded paired checkpoint and exact authenticated peer
- WHEN all objects are verified and durably published
- THEN delivery MUST NOT authorize resume or cleanup without independent fencing, placement and retirement evidence
