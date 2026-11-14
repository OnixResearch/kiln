## Why

Issue #60 requires four independent observed inventories rather than confusing configured project admission slots with physical or inference capacity. Operators need exact work-specific explanations without acquiring placement, model lifecycle or request scheduling authority.

## What Changes

Add a versioned inventory v2 profile/snapshot admission and read-only projection, exact definition/worker/compute/serving bindings, bounded work readiness explanations, strict provenance/freshness and shared-pool accounting. Preserve all existing v1 CI and worker-slot contracts. Add an explicit-snapshot API shell, offline JSON/HTML consumer and accessible console panels. Revision preparation/selection/pinning, task-family qualification, policy reasons and unknown usage remain separate supplied facts.

## Impact

Consumer: operators viewing admitted tenant inventories and exact work attempts. Maintenance owner: Kiln core/provider/web maintainers. Repeatability: focused pure Rust and Node behavioral cases plus the same admitted projection across live and offline consumers. Provider observations and authorizations must be independently qualified; fixtures prove no live fleet. No scheduling, promotion, deployment, discovery, credentials or remote changes.
