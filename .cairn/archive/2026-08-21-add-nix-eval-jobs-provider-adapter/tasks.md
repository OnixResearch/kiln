# Tasks: add a nix-eval-jobs provider adapter

## Phase 1: Reference, baseline, and profile

- [x] [serial] I1 Record `NixOS/nix-eval-jobs` revision `a0cd02231c58974a6b5aaa3712069b071047162e`, GPL-3.0 license, build inputs, Nix cohort behavior, output fields, selected concepts, rejected semantics, and non-claims. r[kiln.nix_eval_jobs.dependency_boundary] r[kiln.nix_eval_jobs.onix_boundary]
- [x] [serial] I2 Run current provider-port, workflow, task, attempt, and standalone conformance tests before core changes. r[kiln.nix_eval_jobs.conformance]
- [x] [serial] I3 Define typed Nickel contracts for executable identity, cohort, source, selector, arguments, environment, bounds, timeout, teardown, partial admission, cache mapping, GC roots, and non-claims. r[kiln.nix_eval_jobs.cohort_admission] r[kiln.nix_eval_jobs.input_admission]
- [x] [parallel] I4 Add positive complete-profile fixtures and negative missing-field, zero-bound, contradiction, unknown-field, binary-drift, version-drift, floating-source, and unsupported-cohort fixtures. r[kiln.nix_eval_jobs.conformance]
- [x] [serial] I5 Add dependency gates that reject Nix, Hydra, and `nix-eval-jobs` implementation imports from core and standalone crates. r[kiln.nix_eval_jobs.dependency_boundary]

## Phase 2: Pure schema and task-mapping cores

- [x] [serial] I6 Define bounded adapter profile, request, external identity, stream record, job result, cache advisory, process fact, GC-root fact, terminal outcome, and task-proposal types. r[kiln.nix_eval_jobs.stream_admission] r[kiln.nix_eval_jobs.task_mapping]
- [x] [serial] I7 Implement pure profile and request admission for exact cohorts, immutable sources, selectors, options, and named limits. r[kiln.nix_eval_jobs.cohort_admission] r[kiln.nix_eval_jobs.input_admission]
- [x] [serial] I8 Implement pure upstream-record schema admission with stable malformed, duplicate, unsupported, conflicting, and exceeded-bound diagnostics. r[kiln.nix_eval_jobs.stream_admission]
- [x] [serial] I9 Implement pure terminal completeness and failure-scope classification over supplied stream, process, teardown, and GC-root facts. r[kiln.nix_eval_jobs.terminal_completeness] r[kiln.nix_eval_jobs.failure_scope]
- [x] [serial] I10 Implement pure successful-job mapping into task proposals with BLAKE3 identity preimages, dependency checks, and separate external identities. r[kiln.nix_eval_jobs.task_mapping]
- [x] [serial] I11 Implement pure cache advisory mapping and reject conflicting deprecated or current fields. r[kiln.nix_eval_jobs.cache_advisory]
- [x] [serial] I12 Implement pure partial-admission and GC-root handoff decisions without filesystem or provider effects. r[kiln.nix_eval_jobs.gc_root_handoff] r[kiln.nix_eval_jobs.core_shell_boundary]
- [x] [parallel] I13 Add property tests for deterministic mapping, stable ordering, checked arithmetic, duplicate rejection, dependency cycles, terminal exclusivity, and equivalent-fact replay. r[kiln.nix_eval_jobs.conformance]

## Phase 3: Optional process and GC-root shell

- [x] [serial] I14 Add executable identity and version preflight against one absolute configured path without `PATH` fallback. r[kiln.nix_eval_jobs.cohort_admission]
- [x] [serial] I15 Add bounded process launch with explicit argv, environment, current directory, timeout, cancellation, stderr capture, termination, kill, and reap. r[kiln.nix_eval_jobs.terminal_completeness]
- [x] [serial] I16 Add incremental complete-line reads under named line, aggregate-byte, record-count, and diagnostic bounds. r[kiln.nix_eval_jobs.stream_admission]
- [x] [serial] I17 Add a private run-scoped GC-root directory, creation observations, downstream handoff, retention, and explicit cleanup outcomes. r[kiln.nix_eval_jobs.gc_root_handoff]
- [x] [serial] I18 Convert shell observations through pure decisions before submitting canonical Kiln commands or build-provider effects. r[kiln.nix_eval_jobs.core_shell_boundary]
- [x] [parallel] I19 Add deterministic fixture processes for valid streams, job errors, malformed lines, truncation, output floods, stderr floods, crash, signal, timeout, cancellation, late output, failed reap, and GC-root failure. r[kiln.nix_eval_jobs.conformance]

## Phase 4: Provider integration and compatibility

- [x] [serial] I20 Register the optional workflow-discovery capability and keep source, build, cache, status, evidence, and gate ports independent. r[kiln.nix_eval_jobs.dependency_boundary]
- [x] [serial] I21 Admit complete task proposals through the canonical Kiln domain transition without direct shell mutation. r[kiln.nix_eval_jobs.task_mapping]
- [x] [serial] I22 Preserve per-job failures, fatal provider errors, timeouts, cancellations, and unknown outcomes as distinct events and projections. r[kiln.nix_eval_jobs.failure_scope]
- [x] [serial] I23 Expose cache observations only as bounded advisory metrics or scheduling facts. r[kiln.nix_eval_jobs.cache_advisory]
- [x] [serial] I24 Add an exact pinned upstream integration fixture and a negative Onix-only `builtins.wasm` cohort fixture. r[kiln.nix_eval_jobs.onix_boundary]
- [x] [parallel] I25 Run shared provider conformance for every declared adapter capability and prove standalone runtime independence. r[kiln.nix_eval_jobs.conformance]

## Phase 5: Documentation and completion evidence

- [x] [serial] I26 Document setup, profile fields, executable provenance, source admission, stream behavior, task mapping, cache advisories, GC roots, partial outcomes, cohort limits, and non-claims. r[kiln.nix_eval_jobs.nonclaims]
- [x] [serial] I27 Add `nix-eval-jobs` to the repository reference section with its exact reviewed revision and adapter-only role. r[kiln.nix_eval_jobs.dependency_boundary]
- [x] [serial] V1 Rerun every Phase 1 baseline command and all focused positive and negative suites. r[kiln.nix_eval_jobs.conformance]
- [x] [serial] V2 Run formatting, workspace tests, strict Clippy, core `no_std` checks, Nickel checks, dependency gates, process-leak checks, and `jj diff --check`. r[kiln.nix_eval_jobs.core_shell_boundary]
- [x] [serial] V3 Run Cairn validation, proposal, design, and tasks gates plus traceability coverage. Record exact output before sync and archive. r[kiln.nix_eval_jobs.conformance]
- [x] [serial] V4 Sync the accepted specification, archive the completed change with evidence, and rerun post-archive validation. r[kiln.nix_eval_jobs.conformance] r[kiln.nix_eval_jobs.nonclaims]
