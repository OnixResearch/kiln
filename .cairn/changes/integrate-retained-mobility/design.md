## Context

The existing hosted runner already owns CH launch, API access, confinement, journal-before-cleanup and capacity. Existing pure retained, handoff and bridge types bind exact attempts and placements. A local marker is not distributed authority.

## Goals and Success Contract

Execute only exact current authorized operations; journal before effects; never replay uncertain migration bytes; keep unknown cleanup occupied. Transfer mutable disk/state only at one paused boundary, verify bounded streamed objects and fsync before publication. Deny live migration unless exact runtime/hardware and resource fencing are qualified.

## Decisions

Reuse existing CH plans and host effects. Keep host API private and verify service-owned directories/sockets before use. Operation-bound controls persist requested and in-progress records before effects; only exact native VMM readback plus confinement/backend barriers adopts observed state. Duplicate calls cannot redispatch, and pending/uncertain controls preserve occupied capacity and block conflicting commands. Use authenticated Iroh connections, bounded streams with backpressure, half-close and deadline, and operation admission before socket use. Retain transfer and placement receipts separately. No boolean transport success grants cutover. Aspen ownership/resource fencing, guest reconnect, cross-host streaming and two-host qualification remain external blockers, never local fallbacks.

## Risks and Trade-offs

Aspen #15 currently lacks qualified cross-host streaming/placement authority and actual resource fencing; two-host KVM qualification requires operator-owned compatible hosts. Unsupported capabilities remain denied, not simulated.

## Verification Strategy

Parent runs focused microvm, hosted and Iroh tests after integration, plus prerequisite-required two-host qualification with exact CH/Iroh/Molten identity. Positive/negative fixtures cover stale commands, crash/replay, ordinary nonzero sender failures, incomplete copies and retirement. No runtime campaign or deployment during implementation.
