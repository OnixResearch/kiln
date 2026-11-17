# Parallel Frontend Evaluation Specification

## Purpose

Defines the `parallel-frontend-evaluation` capability.

## Requirements

### Requirement: Opt-in parallel frontend evaluation

r[kiln.build.parallel_frontend_evaluation] Kiln MUST provide an opt-in, pinned-nightly-only, per-job resource-limited benchmark for Rust parallel frontend threads without changing default builds. It MUST compare isolated latency with multiple independent concurrent jobs and retain exact compiler, flags, source, limits, process-tree cgroup counters, queue wait, wall time, failure and OOM observations. Analysis MUST compare threads within the same nightly and concurrency, refuse recommendations with insufficient samples, and retain repeat-build artifact hashes and reproducibility limitations.

#### Scenario: Pinned nightly and unchanged baseline

- GIVEN a named nightly and the repository's pinned stable baseline
- WHEN controlled check jobs are prepared from warm dependencies and measured
- THEN only the selected crate receives `-Zthreads=N`, the stable baseline omits `-Z`, and raw rows identify both exact toolchains, commands, controls and host.

#### Scenario: Multiple workers under explicit limits

- GIVEN at least two independent concurrency levels with per-job CPU and memory limits
- WHEN each job runs in a private target and transient user scope
- THEN queue wait, wall time, cgroup CPU usage, memory peak, memory OOM events, exit and bounded stderr are retained, including failed jobs; no global jobserver budget is claimed.

#### Scenario: Decision respects evidence limits

- GIVEN repeated raw rows for each nightly thread/concurrency cell
- WHEN pure analysis evaluates distributions and throughput
- THEN cells with fewer than three successes are insufficient and cannot win a recommendation, and a nightly winner is recommended only if its throughput exceeds threads=1 beyond observed spread.

#### Scenario: Repeat build hash and feature omission

- GIVEN two matching build-mode repeats and available `b3sum`
- WHEN the produced library artifact is hashed
- THEN equality or inequality is reported without promising cross-host reproducibility, and operators can omit the script and all `-Z` flags to retain default builds.
