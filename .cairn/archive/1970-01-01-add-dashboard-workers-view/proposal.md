## Why

The operator needs to see available execution targets and busy work without inferring worker health from the number of configured projects. The live workspace has queue/provider observations but no Workers page or heartbeat registry.

## What Changes

- Add a read-only authenticated Workers view over configured workflow targets, grouping shared provider/endpoint/platform bindings without exposing endpoint paths.
- Show platform, assigned and enrolled projects, configured execution slots, occupied/free slots, queued work, and exact latest retained run/provider observations.
- Support metadata search, slot-state filtering, URL-restored exact selection, refresh, mobile navigation and safe session clearing. Explicitly distinguish free admission slots from unknown worker liveness.

## Impact

Kiln web-service/dashboard maintainers own the projection and UI. The existing self-CI operator is the immediate consumer; the durable capability is a bounded, reproducible target inventory using existing service configuration and retained state. No provider discovery protocol, worker registration, heartbeat, scheduling policy, canonical profile migration, deployment, lifecycle acceptance or source publication is introduced. Verification exercises occupancy, queue, unknown/recovery, authentication/redaction, exact navigation and actual desktop/mobile rendering.
