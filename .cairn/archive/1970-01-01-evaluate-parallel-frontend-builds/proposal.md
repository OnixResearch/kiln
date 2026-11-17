# Why

Issue #88 needs a bounded, reproducible decision about opt-in Rust parallel frontend threads under independent agent-worker contention. A single isolated build cannot establish cross-worker throughput or whether extra compiler threads help when CPU and memory are constrained.

## What Changes

- Add a standard-library-only opt-in benchmark runner and pure analysis with positive and negative unit tests.
- Retain real per-job cgroup CPU/memory and timing observations, same-nightly thread comparisons, and repeat-build artifact hashes. Measure rustc itself: no configured compiler-cache wrapper, no incremental state and a recorded Cargo job count.
- Document controlled execution, non-claims and how to omit the feature entirely; the matrix and recommendation remain pending operator execution.

## Impact

- **Files**: `scripts/parallel-frontend-bench.py`, `scripts/parallel_frontend_analysis.py`, `scripts/test_parallel_frontend.py`, `docs/parallel-frontend-evaluation.md`, `cairn-policy/development-coverage.ncl` (active delta root), and this Cairn package.
- **Testing**: Python unit tests and Cairn proposal/design/tasks gates; do not run a benchmark or Cargo build during implementation. Default toolchain and build flags remain unchanged.
