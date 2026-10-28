## Why

The maintained single-tenant service has source-bound execution and useful UI inspection, but its operated runs are single-node without recorded node durations. Worker inventory describes configured admission slots rather than measured compute; reviewed worker commands have host authority; caching and developer submission are not integrated.

## What Changes

Implement the four user-selected internal-service workstreams: real multi-node execution and telemetry/reports; heartbeat/capacity observations and explicitly isolated jobs with cancellation/recovery evidence; provider-backed shared cache with bounded verified transfers and cold/warm observations; developer submit/watch/logs/cancel/retry plus exact, explicit dirty-worktree inputs.

## Impact

Affected capability: internal-developer-loop. Kiln web-service composition and CLI own adoption. Kiln maintainers own maintenance. Existing Git/Lattice/Bounded Exec ports remain authoritative; no provider implementation enters the pure core. Repeatability requires disposable real service/worker exercises and retained command results. Historical unknown timing stays unknown and unchanged optional configuration preserves existing profile identities.

## Out of Scope

GitHub Actions compatibility, hosted tenancy/billing, interactive SSH debugging, remote publication, accepting legal terms, privileged host changes, production state deletion, and automatic deployment or workflow/profile migration. No claim of arbitrary-code safety, complete sandbox security, or CI success based on telemetry/cache observations.
