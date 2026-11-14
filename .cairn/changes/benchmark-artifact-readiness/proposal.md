## Why

Issue #59 requires measurement before optimization: cache-hit labels and byte counts conceal readiness work and runner mixes. No benchmark result currently qualifies a performance claim.

## What Changes

Add typed bounded workloads, an opt-in hosted-cache observation harness, a real Nix command rail, and independent raw-sample analysis. Predeclare an optimization decision and minimum evidence. Keep missing phases and unavailable tiers absent; retain failures and CPU separately from elapsed time.

## Impact

- Consumer: Kiln maintainers evaluating artifact-readiness costs.
- Owner: hosted-cache and Nix adapter maintainers.
- Files: dedicated benchmark scripts/workloads/docs, hosted cache observation helpers and example, this lifecycle package.
- Authority: local implementation only; parent coordinates checks and explicitly selects disposable benchmark roots. No remote execution, shared GC, service restart or publication.
- Non-claims: no production qualification, remote-tier parity, compiler speedup or performance result before a recorded real campaign.

## Out of Scope

No changes to keys, admission, retries, fencing, teardown, publication ordering or VM/session execution. No telemetry backend or cache optimization.
