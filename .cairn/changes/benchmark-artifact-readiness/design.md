## Context

The existing hosted cache returns native outcome, bytes, duration, identity and source tier. Its supported regular-file format rejects symlinks. Nix supports real file/symlink/reference cohorts. Provider operations whose internals are not observable remain explicitly coalesced, not invented subphase timings.

## Goals and Success Contract

One bounded command retains exact workload, revision, runner CPU/storage/profile, cold/warm controls, raw successful and failed attempts, elapsed and CPU observations, and independent distribution analysis. Equal-content-byte workloads vary file count, symlinks and shared references. A named injected phase delay must remain outside compilation and inside its actual phase. Planning gates precede product edits; real campaigns are coordinated by the parent.

## Decisions

### Decision: Preserve existing provider semantics

Compile observation hooks only with a benchmark feature. Scoped thread-local recording and optional delay wrap existing synchronous cache operations without replacing decisions or changing operation order. Normal builds inline a direct operation. The harness invokes actual restore/publish after successful owned work, not a second cache implementation. Unsupported symlink cache cohorts are explicit absent observations and use Nix instead.

### Decision: Bounded real paths and honest controls

The hosted runner owns fresh source directories and a private cache; cold means empty owned application cache, never dropped kernel pages. Warm uses the same exact input/profile identity and a fresh destination. Nix performs actual evaluation, dry-run planning, realization and NAR operations with existing tools and a caller-selected immutable builder closure; no ambient download or shared GC. Native realization can coalesce compilation/substitution/import and is labeled accordingly. No VM execution is added.

### Decision: Predeclare evidence before measurement

Evaluate only the proposal to reduce regular-file materialization overhead by changing archive layout. Go requires at least 30 complete comparable samples per cell, p95 at least 20% and 5 ms lower, unchanged payload/output semantics, no failure regression, and a separately reviewed complexity/retention budget. Otherwise no-go; missing evidence is insufficient, not success. p95 requires 20 and p99 100 observations; small samples have median/range and ECDF only. Analysis groups by provider/profile/runner/workload/cache state/delay and never mixes failures into success percentiles or discards them.

## Verification Strategy

After the native planning gates, implement valid and invalid workload/analysis cases plus phase-attribution tests. Parent runs focused tests and the smallest cohort in an owned /run directory. Raw observations and independent analysis are retained; no tests, builds or benchmarks run mid-flight. Runtime evidence tasks remain unchecked until exercised.

## Risks and Trade-offs

Kernel page cache is not controlled. Coalesced provider operations cannot identify downloader/decompressor internals. CPU scope excludes daemon work for Nix and is explicitly labeled. Bounds and artifact retention prevent unbounded samples; no source-wide or remote qualification follows from one runner.
