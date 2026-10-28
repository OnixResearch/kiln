## Context

The current service retains exact Git/Lattice source and execution identities in SQLite, uses bounded HTTP transports, and supervises reviewed child commands through Bounded Exec. The user selected all four recommended internal-service workstreams rather than hosted multi-tenant product scope.

## Decisions

### Decision: Preserve exact existing identities

Use separately admitted runtime policy or optional fields omitted when absent. Never replace an installed workflow/profile binding by editing stored identities or deleting history. New dirty-worktree inputs have explicit content identity and provenance, never masquerade as the original committed source. Bound and authenticate all uploads/downloads, reject symlinks and traversal, and verify content before execution.

### Decision: Keep mechanisms behind shells

Reuse the Git, Lattice, Bounded Exec, report parsers and existing source/profile admission. Pure admission and projection functions take explicit facts. Shells own clocks, processes, namespaces, filesystem, HTTP and SQLite. Isolation is opt-in reviewed policy with preflight fail-closed when host capabilities are absent; never silently run unsandboxed. Observed CPU/memory and worker liveness are observations, not guaranteed scheduling or isolation proofs.

### Decision: Independent owners with one integrator

Pipeline owner owns telemetry contracts, model additions, telemetry API module, pipeline fixtures/exercise and UI telemetry presentation. Worker owner owns execution/isolation module, process supervision integration, worker heartbeat module, worker profile and its Nickel contract. Cache owner owns cache profile/store, engine cache API and worker cache module. Developer owner owns developer CLI, explicit snapshot module/API, worker source acquisition and queue input admission. Parent owns shared module wiring, manifests, server/main/API routing, root worker orchestration, lifecycle, operator documentation and integrated verification. Owners coordinate exported interface additions before shared files change; no concurrent shared-file edits. Child tasks skip builds/tests/formatters; parent exercises integration once assembled.

### Decision: Exact observation and cache admission

Authenticate worker observations and bind per-step records to exact run/source/attempt/execution/node, with bounded monotonic sequences and idempotency. Worker heartbeats expire by explicit observation windows and do not equate configured slots with compute. Cache keys bind producer/toolchain/platform and declared inputs; restore verifies BLAKE3 and safe paths; bounded retention must never prune source, canonical events or artifacts. Report hits, misses and transferred bytes from actual operations and measure cold/warm execution, without inferring correctness from a cache hit.

### Decision: Developer-facing operations

Provide HTTPS CLI submit, watch, logs, cancel and retry against the existing exact-source/control APIs. Credentials are explicit private-file inputs and never printed, inherited into jobs, or persisted in snapshot data. Mutations are not blindly retried; dirty inputs are explicitly selected, bounded and content-addressed. Watch and logs retain real exit/outcome semantics.

## Risks / Trade-offs

Host namespace/cgroup availability is an external prerequisite, not permission to alter host policy. Remote cache trust and credentials must remain explicit. Single-source existing native handlers may require reviewed offline workflow migration before production multi-node adoption. Record unavailable native prerequisites precisely and do not promote mock tests to runtime qualification. Existing unrelated lifecycle holds are preserved.

## Verification Strategy

Validate proposal, design and task gates before implementation. Establish focused existing behavior baseline, then run paired contract/error tests and real disposable HTTPS service/worker/CLI exercises: branching dependencies and failure; real step timing/resource reports; stale heartbeat and mismatched callback rejection; isolated access denial, timeout, cancellation and restart reconciliation; verified cache cold/miss and warm/hit with corruption rejection; clean versus dirty source identities and exact execution; CLI logs/control/retry. Check desktop/mobile UI on actual data. Run repository-required tests, lint, lifecycle and Nix gates and record exact outcomes, preserving failures and non-claims.
