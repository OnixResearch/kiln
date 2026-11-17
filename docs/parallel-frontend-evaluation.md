# Opt-in parallel Rust frontend evaluation

Issue #88's decision is whether a pinned nightly's experimental frontend
parallelism improves an agent-worker edit/rebuild loop under contention. This is
an **evaluation harness, not a production build setting or a speedup claim**.
No matrix was run while implementing it. The default Rust 1.88.0 toolchain,
Cargo configuration, Nix defaults and production build flags are unchanged.

## Prerequisites and command

Use a quiet Linux host with cgroup v2, a working `systemd --user` manager and
`systemd-run --user --scope` from systemd 254 or newer (for
`--expand-environment=no`), Rustup with the exact dated nightly and pinned
1.88.0 installed, Cargo dependencies already available, `cp` supporting
`--reflink=auto`, free disk for two warm templates plus one private target per
concurrent job, Python 3,
and `b3sum` when using build mode. The host operator must ensure the user scopes
can set CPUQuota and MemoryMax; if the manager refuses, results remain failed
rows rather than silently falling back to an unbounded run. Use a fixed source
checkout; pass its exact revision explicitly. The output leaf must not exist.
Run from the workspace root:

```console
python3 scripts/parallel-frontend-bench.py \
  --toolchain nightly-2026-07-22 --baseline-toolchain 1.88.0 \
  --crate kiln-web-service --threads 1,2,4,8 --concurrency 1,2,4 \
  --repeats 3 --cpu-quota 200% --memory-max 4G \
  --source-revision <exact-checked-out-revision> \
  --out /run/user/$(id -u)/kiln-parallel-frontend-check
```

`--toolchain` requires a dated nightly, not floating `nightly`, `stable` or
`beta`. `--threads` accepts unique values from 1,2,4,8 (including 1),
`--concurrency` unique values from 1,2,4,8 (including 1 and at least one other
level), and `--repeats` 1..100. Each concurrently running job has its **own**
CPUQuota and MemoryMax, with MemorySwapMax=0. For instance 200% is two CPUs
per job, **not** a shared two-CPU budget across workers. The Cargo/rustc
jobserver coordinates processes within a build process tree but cannot enforce
a global budget across independent agent-worker scopes. Keep the same host,
quota, memory limit and source for comparable cells; record competing host load
separately if it cannot be excluded.

Every Cargo invocation, including template warmup and the measured jobs, runs
with `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` set empty, `CARGO_INCREMENTAL=0`
and `CARGO_BUILD_JOBS` from `--cargo-jobs` (default: the host CPU count). An
empty wrapper overrides a `build.rustc-wrapper` from Cargo configuration, so a
compiler cache such as kache or sccache cannot replace measured compilation
with cache hits. Rows retain these values in `cargo_environment`, and analysis
treats `cargo_jobs` as a control. Other inherited configuration, such as target
`rustflags`, stays in effect and appears in the verbose `rustc_commands`.

For the separate artifact-hash repeat check use a new output leaf and at least
two repeats:

```console
python3 scripts/parallel-frontend-bench.py \
  --toolchain nightly-2026-07-22 --mode build --repeats 3 \
  --cpu-quota 200% --memory-max 4G \
  --source-revision <exact-checked-out-revision> \
  --out /run/user/$(id -u)/kiln-parallel-frontend-build
```

Build mode refuses to start without `b3sum`. It hashes the selected crate's
`.rlib` after each successful dev-profile repeat; `artifact_hash_pair` and
`artifact_hash_equal` in analysis compare the first two repeat hashes within
the same cell. The target directory and unrelated artifacts are **not** hashed.
A mismatch needs investigation; an equality is only local repeat evidence on
one source/toolchain/host, not a deterministic-nightly or cross-host guarantee.
Nightly flags, metadata paths, build scripts, linker inputs and compiler
versions can change; never compare hashes across compiler or source cohorts.

## Measured boundary and retained data

For each toolchain the runner first warms a template target with `cargo check`
(or `cargo build` for build mode). For each independent job it copies that
warmed target using `cp -a --reflink=auto` to a private directory and invokes
`cargo clean -p <crate> --target-dir <private>`; only the selected crate is
cleaned, preserving dependency fingerprints. Setup/copy/clean and the initial
warmup are **outside** the timed batch. `cargo rustc --profile check -p <crate>
--lib -- -Zthreads=N` measures the nightly crate; `--profile dev` measures build
mode. Other dependencies do not receive `-Zthreads`; the stable 1.88.0 baseline
omits every `-Z` flag. The actual argv also includes `--locked`, `-vv`,
target-dir and JSON message format, all recorded verbatim. Cargo's verbose
`Running` lines retain the full observed compiler invocations and flags.

A transient user scope runs each job and captures its own cgroup v2 `cpu.stat`
`usage_usec`, `memory.peak` and `memory.events` `oom_kill` before exit. The
runner passes `--expand-environment=no`: by default `systemd-run` substitutes
`${...}` in its arguments, which would erase the wrapper's cgroup path and make
it copy the root cgroup's counters. These
are scope process-tree observations, not Python launcher CPU. Separately the
launcher samples resident pages from every PID in the scope's `cgroup.procs`
every 50 ms; `peak_rss_bytes` is the maximum sampled process-tree RSS and
`rss_samples` records coverage. Short-lived peaks may be missed; `memory.peak`
is an authoritative cgroup charged-memory high-water mark, **not RSS** (it can
include cache and kernel accounting). A missing sample is null, not zero.
The wrapper records start/end timestamps around Cargo. `queue_wait_s` is the elapsed time
from batch dispatch to wrapper start (including systemd launch); it is **not**
an external scheduler admission queue. `wall_s` measures Cargo's execution,
including rustc and dependencies that actually rebuild; `end_to_end_s` also
includes the observed launch/queue interval from batch dispatch to job finish.
The batch makespan includes launch and the slowest completion, and throughput
is successful completed jobs per minute across measured batch makespans. Tail
latency is nearest-rank p95 wall; p95 on three observations is only the maximum
of those observations, not a reliable population tail estimate.

`jobs.jsonl` retains all jobs including failures, with schema
`kiln.parallel_frontend.job.v1`: toolchain name, full `rustc -vV`, exact scope
and cargo argv, verbose `rustc_commands`, crate, mode, threads, concurrency,
repeat, batch ID,
source revision, host CPU count and memory, per-job CPU/memory/swap limits,
queue wait, execution wall, end-to-end wall, CPU seconds, cgroup charged-memory
peak, sampled peak RSS and count, OOM kills, exit status, bounded stderr tail
and optional artifact path/BLAKE3 hash. Missing measurements are `null`, not
fabricated zeros. Per-job directories retain cgroup source files. Each job's
private target is removed once the job is measured and hashed, and each
template is removed after its toolchain's batches, so a retained
`artifact_path` names a removed file. No cleanup of external Cargo caches or
shared stores occurs.
If setup fails before a batch, the command exits nonzero; completed raw rows
remain. An interrupted batch might have no rows for jobs not yet finalized.

Run the pure analyzer independently (no Cargo or scopes):

```console
python3 scripts/parallel_frontend_analysis.py /path/to/output/jobs.jsonl
python3 -m unittest scripts/test_parallel_frontend.py
```

`analysis.json` uses schema `kiln.parallel_frontend.analysis.v1`. Each cell
contains its exact controls, total n/successes/failures/OOM kills, execution
wall/end-to-end wall/cgroup memory peak/sampled peak RSS/queue distributions
(`n`, median, nearest-rank p95, min, max), mean CPU seconds, mean
`cpu_seconds/(wall_s*quota_cpus)`, aggregate completed jobs/min,
per-batch throughput spread, insufficiency marker and optional repeat-hash
pair/equality. CPU utilization can exceed one if accounting boundaries or
quota assumptions differ; it is a ratio, not clamped. Null counters produce
null aggregates, not zero. Analysis never pools distinct toolchains, compiler
versions, source revisions, modes, limits or hosts.

`recommendations` contains per-nightly/control/concurrency candidate outcomes.
At least three successful samples **and two complete batches** are needed for
both threads=1 and a candidate, with resource counters present for every job.
The highest aggregate-throughput candidate is recommended only if its minimum
observed per-batch throughput exceeds the maximum observed threads=1 throughput.
Otherwise status is `no recommendation`.
This conservative observed-spread check is not a statistical significance test.
Only threads on the **same nightly** isolate the parallelism effect; comparing
nightly directly to 1.88.0 also changes the compiler and is descriptive only.
Failures/OOMs remain visible and are never converted to successes or imputed
into latency. Operators should inspect them before acting even when a cell has
three successful samples.

To disable/omit the experiment, do not invoke this script and use the ordinary
pinned-toolchain `cargo check`/`cargo build`. There is no feature to toggle in
production, no persistent environment setting, and no `-Zthreads` in normal
builds. The parent will run and retain a controlled matrix later, then write a
recommendation or explicitly decline one based on its evidence.
