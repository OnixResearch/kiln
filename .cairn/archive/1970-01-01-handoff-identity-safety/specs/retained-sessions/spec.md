# Paused Handoff Identity Delta

## MODIFIED Requirements

### Requirement: Ordered paused handoff

r[kiln.hosted.handoff.order] A paused handoff MUST persist the full immutable originating session identity (session, VM, work and revision, attempt, fence, execution) and source/target placement together with its operation. Every phase transition, reconciliation transition and retirement observation MUST compare the supplied session identity and source placement to that persisted origin after validating the supplied owner. Serialized reopen MUST preserve these bindings; a record without the origin identity MUST fail closed. Every mismatch MUST leave phase and evidence unchanged.

Object verification MUST compare caller-supplied independently resolved session, VM, work/revision, attempt/fence, execution, source/target and operation facts to the persisted handoff and MUST pass those expectations unchanged to checkpoint admission. It MUST NOT normalize expected facts from the session or handoff. Checkpoint admission MUST reject direct and wrapped mismatches.

Target admission and resume MUST bind the exact operation, complete admitted manifest identity, source and target placement, target owner, durable placement acknowledgement, and source volume/effect fencing observations. A native receipt reference alone MUST NOT satisfy these facts. Refusal MUST leave the serialized state unchanged. These pure checks MUST NOT claim that a provider receipt is truthful or that resource fencing is effective.

#### Scenario: Complete handoff survives serialized reopen
r[kiln.hosted.handoff.order.scenario.complete]

- GIVEN an admitted paused source and exact independent checkpoint facts
- WHEN every ordered transition is performed under the originating owner, serializing and reopening between phases
- THEN the exact manifest and cutover evidence remain bound and target resume precedes source retirement.

#### Scenario: Caller B cannot advance caller A's handoff
r[kiln.hosted.handoff.order.scenario.order]

- GIVEN a handoff begun with session A
- WHEN any phase, reconciliation or retirement call supplies independently authorized session B, including after serialized reopen, with matching or changed placement
- THEN it MUST refuse and leave phase, manifest, receipt and evidence unchanged.

#### Scenario: Changed origin fields and crossed operation are refused
r[kiln.hosted.handoff.order.scenario.order]

- GIVEN a handoff with a persisted original identity and operation
- WHEN a later call changes session, work, revision, attempt, fence, execution, VM, placement host/generation or operation binding
- THEN it MUST refuse without advancing or replacing evidence.

#### Scenario: Independently resolved manifest facts are not normalized
r[kiln.hosted.handoff.manifest.scenario.disk]

- GIVEN a manifest and expectation that differ in execution or any session/work/revision/attempt/fence/placement/operation binding
- WHEN direct checkpoint admission or the handoff object-verification transition runs
- THEN admission MUST fail and the handoff phase and stored manifest MUST remain unchanged.

#### Scenario: Stale or forged target evidence is refused
r[kiln.hosted.handoff.order.scenario.order]

- GIVEN verified objects and target evidence
- WHEN cutover operation, manifest scope, placement, target owner or volume/effect fencing facts are stale, absent or mismatched
- THEN target admission/resume MUST refuse and preserve the previous phase and evidence.
