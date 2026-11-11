# Retained Sessions Delta Specification

## ADDED Requirements

### Requirement: Checkpoint manifest admission

r[kiln.hosted.handoff.manifest] A paused handoff MUST admit one checkpoint
manifest binding the exact session, work/version, attempt/fence, execution,
source/target placement, operation, image/kernel/hypervisor/CPU profile, disk
generation and distinct RAM/device-state/disk/workspace objects. Admission MUST
refuse mismatched bindings, equal source/target hosts, non-advancing target
generations, invalid identities, duplicate objects, and a disk generation that
differs from the independently resolved volume generation. A manifest MUST NOT
be treated as proof that objects were transferred, verified or durably stored.

#### Scenario: Old memory with a new disk

r[kiln.hosted.handoff.manifest.scenario.disk]
- GIVEN a manifest whose memory is paired with a different disk identity or generation
- WHEN checkpoint admission runs
- THEN it MUST fail before transfer or target admission

#### Scenario: Uncertain transfer needs reconciliation

r[kiln.hosted.handoff.manifest.scenario.reconcile]
- GIVEN an admitted handoff with an uncertain final acknowledgement
- WHEN reconciliation is required
- THEN the handoff MUST enter reconciliation without claiming retirement or releasing source capacity

### Requirement: Ordered paused handoff

r[kiln.hosted.handoff.order] A paused handoff MUST order intent, quiescence,
source pause, object verification, target admission, target resume and source
retirement. Object verification MUST require a paused source and the exact
manifest. Target admission MUST require the admitted target owner and a validated
receipt. Resume MUST require the admitted target placement. Retirement MUST
follow resumed target ownership. Every command MUST carry the current session,
fence, host and generation; stale or out-of-order commands MUST fail.

#### Scenario: Complete ordered handoff

r[kiln.hosted.handoff.order.scenario.complete]
- GIVEN a paused source and an admitted manifest
- WHEN each ordered command runs under its current owner
- THEN target admission and resume MUST precede source retirement

#### Scenario: Out-of-order or stale command

r[kiln.hosted.handoff.order.scenario.order]
- GIVEN verification before source pause, a stale target generation, or retirement before target resume
- WHEN the command runs
- THEN it MUST fail and leave the current phase unchanged
