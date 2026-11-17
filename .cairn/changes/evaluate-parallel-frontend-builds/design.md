# Context

The repository pins Rust 1.88.0. Parallel frontend threads require a named nightly and `-Zthreads=N`, and the effect is experimental. Cargo's jobserver coordinates processes inside one build tree, not independent workers.

## Decisions

### Decision: warm private targets and scope each measured job

Prepare one checked/build-warmed target template per toolchain, clone it using `cp -a --reflink=auto`, and clean only the selected package in each private target. Time `cargo rustc --profile check -p <crate> --lib -- -Zthreads=N` under a transient user systemd scope, or `--profile dev` for artifact hashing. Baseline omits `-Z`. Each scope applies CPUQuota, MemoryMax and MemorySwapMax=0 and captures its own cgroup counters before exiting. Setup is excluded from the measured batch interval; queue wait measures launch-to-wrapper-start, not an external scheduler.

### Decision: measure rustc itself under one recorded Cargo environment

Every Cargo invocation sets `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` empty, `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS` from `--cargo-jobs`. Cargo treats an empty wrapper variable as overriding a configured `build.rustc-wrapper`. On the development host, user Cargo configuration routes rustc through a kache wrapper; without the override, measured compilations could be cache hits. Rows retain the controlled values, and analysis groups by `cargo_jobs`. Each job's private target is removed after measurement and hashing, and each template after its toolchain's batches, so disk use is bounded by two templates plus one batch.

### Decision: retain raw failures, analyze matched cells only

JSONL rows record exact compiler and argv, environment-relevant limits, source identity, host facts, timing, resource counters and bounded stderr. Missing counters are null, never zeros. The pure analyzer groups by toolchain, mode, threads, concurrency and controls, reports successful distributions and failed/OOM counts, and recommends only from three or more successes per cell on the same nightly and concurrency. A winner must beat threads=1 beyond observed throughput spreads; otherwise there is no recommendation.

### Decision: build artifact repeat check stays distinct

Build mode requires `b3sum` and at least two repeats; hash the selected produced library artifact per repeat, retaining artifact path/hash and pairwise equality. The hash checks an artifact, not a bit-for-bit full target directory or deterministic Rust compiler claim.

## Risks / Trade-offs

- cgroup v2 and a working user systemd manager are prerequisites; unavailable counters become missing evidence and prevent strong recommendations.
- CPUQuota throttling, host co-tenants, template copy cost and source drift can confound comparisons; retain limits and host facts, run on a quiet machine and hold controls fixed.
- Nightly compiler behavior is unstable; reproducibility equality on one host does not imply cross-host reproducibility.
