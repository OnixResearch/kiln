## Phase 1: Reference boundary and typed policy

- [x] [serial] Record Pueue revision `b3fe223ef0a3c51d95d5c6128700b79442c54af7`, MIT license, selected operator concepts, rejected runtime concepts, maturity, and non-claims in lifecycle evidence and documentation. r[kiln.task_operations.reference_boundary] r[kiln.task_operations.nonclaims]
- [x] [depends:reference-record] Add dependency and compatibility gates that allow documentation references but reject Pueue crates, imports, executable calls, protocols, feature names, provider profiles, adapters, fixtures, and generated schemas. r[kiln.task_operations.reference_boundary]
- [x] [serial] Define typed Nickel contracts for status bounds, capacity classes, log segments, retention, cursors, follow policy, wait predicates, deadlines, dependency behavior, delay behavior, controls, and redaction. r[kiln.task_operations.status] r[kiln.task_operations.logs] r[kiln.task_operations.follow] r[kiln.task_operations.wait] r[kiln.task_operations.capacity] r[kiln.task_operations.blocking_facts] r[kiln.task_operations.controls] r[kiln.task_operations.redaction]
- [x] [parallel] Add positive Nickel fixtures for complete operator, capacity, log, wait, control, and redaction profiles. r[kiln.task_operations.fixtures]
- [x] [parallel] Add negative Nickel fixtures for missing bounds, invalid limits, unsupported predicates, conflicting policies, unsafe fields, and local-capacity promotion into tenant fairness. r[kiln.task_operations.fixtures] r[kiln.task_operations.capacity] r[kiln.task_operations.redaction]

## Phase 2: Pure task-operations core

- [x] [serial] Define bounded readiness, execution, outcome, block-reason, status-record, log-metadata, cursor, wait-selection, wait-predicate, capacity-observation, hold, and control-request types without shell dependencies. r[kiln.task_operations.lifecycle_facets] r[kiln.task_operations.logs] r[kiln.task_operations.wait] r[kiln.task_operations.capacity] r[kiln.task_operations.controls]
- [x] [depends:task-operations-types] Implement pure lifecycle-facet projection through the production event and projection path without a second task state machine. r[kiln.task_operations.status] r[kiln.task_operations.lifecycle_facets]
- [x] [depends:task-operations-types] Implement pure stable status filtering and ordering over exact project, evaluation, run, task, attempt, provider, outcome, and capacity-class selections. r[kiln.task_operations.status]
- [x] [depends:task-operations-types] Implement pure log-cursor, range, truncation, gap, retention, and completion validation over supplied metadata. r[kiln.task_operations.logs] r[kiln.task_operations.follow]
- [x] [depends:task-operations-types] Implement pure wait-predicate evaluation over exact bounded selections and projection snapshots. r[kiln.task_operations.wait]
- [x] [depends:task-operations-types] Implement one pure readiness decision over dependency facts, holds, admitted time, provider-local capacity observations, and policy. r[kiln.task_operations.capacity] r[kiln.task_operations.readiness] r[kiln.task_operations.blocking_facts]
- [x] [depends:task-operations-types] Implement pure admission for hold, release, cancel, provider pause, and provider resume with exact scope, attempt, capability, and stale-observation checks. r[kiln.task_operations.controls]
- [x] [depends:task-operations-types] Implement safe-field diagnostic projection without unrestricted formatting of shell-owned or provider-owned records. r[kiln.task_operations.redaction]

## Phase 3: Operator and log shells

- [x] [depends:lifecycle-projection] Add rebuildable human and versioned machine status output that consumes the same bounded status records and has no mutation authority. r[kiln.task_operations.status] r[kiln.task_operations.lifecycle_facets]
- [x] [depends:log-cursor-core] Add a bounded durable log store with ordered segments, BLAKE3 segment identities, atomic metadata linkage, named retention, and explicit gap and truncation records. r[kiln.task_operations.logs]
- [x] [depends:durable-log-store] Add bounded log read and follow shells with exact stream selection, cursor resume, deadline handling, disconnect handling, and terminal stream reporting. r[kiln.task_operations.follow]
- [x] [depends:wait-predicate-core] Add a wait shell that observes projection changes and admitted deadline facts without mutating task or attempt outcomes. r[kiln.task_operations.wait]
- [x] [depends:readiness-core] Wire scheduler orchestration to the pure readiness decision without shell-local queue authority, clock reads in core, or silent dependency fallback. r[kiln.task_operations.readiness] r[kiln.task_operations.blocking_facts]
- [x] [depends:control-admission-core] Wire admission holds to canonical commands and events, and wire supported cancel, pause, and resume requests to typed provider effects. r[kiln.task_operations.controls]
- [x] [depends:safe-diagnostic-core] Apply safe-field projections and named bounds to CLI, API, status, log metadata, errors, tracing fields, and `Debug` output. r[kiln.task_operations.redaction]

## Phase 4: Recovery and behavioral fixtures

- [x] [depends:durable-log-store] Add restart recovery for committed segments, partial writes, expired retention, missing metadata, stale cursors, and unresolved provider stream observations. r[kiln.task_operations.logs] r[kiln.task_operations.follow]
- [x] [parallel] Add positive core fixtures for ready work, each block reason, lifecycle facets, status ordering, log ranges, cursor resume, follow completion, wait completion, capacity admission, holds, release, cancellation, pause, resume, and safe diagnostics. r[kiln.task_operations.fixtures]
- [x] [parallel] Add negative core fixtures for cycles, dependency failure, missing time, stale time, missing or stale capacity, malformed cursors, stream mismatch, expired data, unsupported controls, stale attempts, alternate-state mutation, secret injection, oversized data, and false non-claims. r[kiln.task_operations.fixtures] r[kiln.task_operations.nonclaims]
- [x] [parallel] Add positive shell fixtures for status rebuild, durable log recovery, bounded follow, wait observation, hold persistence, and admitted provider controls. r[kiln.task_operations.fixtures]
- [x] [parallel] Add negative shell fixtures for interrupted writes, cursor gaps, client disconnect, client deadline, task-timeout confusion, failed provider control, unknown control outcome, redaction failure, unbounded output, and hidden mutation authority. r[kiln.task_operations.fixtures] r[kiln.task_operations.nonclaims]
- [x] [depends:reference-boundary-gate] Add explicit negative fixtures that introduce a Pueue dependency, import, process call, protocol type, provider profile, adapter, and generated schema, and prove that each case fails closed. r[kiln.task_operations.reference_boundary] r[kiln.task_operations.fixtures]

## Phase 5: Documentation and completion evidence

- [x] [serial] Document operator status, lifecycle facets, log and follow cursors, wait predicates, capacity classes, readiness facts, holds, provider controls, recovery, redaction, and non-claims. r[kiln.task_operations.status] r[kiln.task_operations.lifecycle_facets] r[kiln.task_operations.logs] r[kiln.task_operations.follow] r[kiln.task_operations.wait] r[kiln.task_operations.capacity] r[kiln.task_operations.readiness] r[kiln.task_operations.controls] r[kiln.task_operations.redaction] r[kiln.task_operations.nonclaims]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std` core checks, Nickel checks, dependency audits, task-operations fixtures, Cairn validation, Tracey coverage, and focused Nix checks. r[kiln.task_operations.fixtures] r[kiln.task_operations.reference_boundary]
- [x] [serial] Record deterministic evidence for configuration, core decisions, shell recovery, fixture results, dependency independence, reference revision, and bounded non-claims. r[kiln.task_operations.fixtures] r[kiln.task_operations.nonclaims]
- [x] [serial] Sync accepted specifications, confirm complete requirement coverage, archive the change with evidence, and rerun post-archive validation. r[kiln.task_operations.reference_boundary] r[kiln.task_operations.fixtures] r[kiln.task_operations.nonclaims]
