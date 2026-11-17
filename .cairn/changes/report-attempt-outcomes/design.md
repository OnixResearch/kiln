# Attempt outcome reporting design

## Decisions

Extend the existing attempt row and opt-in metric receipt, not a new store.
`ModelTurnObservation` distinguishes `Absent { reason }`, complete
`Observed { turns }`, and `Incomplete { turns, reason }`; turns mean model
requests, never JSONL events or Agent nodes. Old usage rows deserialize absent.

Retain optional `worker_observation` on `ImplementationAttempt` with typed
`WorkerTerminalState` (`Completed`, `Failed`, `Aborted`, `Deadline`), exact worker
profile when known, optional declared harness/model and `AttemptUsage`.
Retain optional `verification_outcome` (`Verified`, `Rejected`, `Unknown`,
`Incomplete`) independently. Missing observations are not fabricated from worker
prose. The accepted existing verification path supplies `Verified`; canonical
controller observations supply rejected/unknown outcomes. Neither field changes
admission, fencing, phase transitions, publication or verification policy.
Observations are exact-fence-bound, idempotent and conflict-refusing. A supported
unknown-to-known verifier reconciliation may refine an unknown observation.

The trusted Lattice shell admits observations only after checking the existing
request/envelope binding. Read bounded native OMP observation artifacts where
available and preserve absent/incomplete telemetry. The currently supported
`lattice.omp-jsonl-observation.v1` has no model request telemetry: its event count
is never a turn count. A versioned measured-usage observation is consumed only
when its attempt/fence/request/profile bindings and coverage are supported;
unsupported cohorts remain explicitly absent, not guessed.

## Report

A pure `AttemptReport::observe(&AttemptLedger, start_ms, end_ms)` computes a
half-open claim-time window `[start_ms, end_ms)`. It includes finished attempts
only in outcome/efficiency denominators, exposes open/excluded/missing samples,
and emits one total plus deterministic profile/harness/model groups. Unknown
profile/model forms an explicit group, not a guessed attribution.

Keep raw numerator and denominator for every rate; zero denominators are absent.
Rejected Completed attempts divide by (1) finished non-verified attempts and
(2) finished Completed attempts. Failure categories are disjoint: deadline,
Completed plus rejected, failed worker, aborted/cancelled, and other/unknown.
Canonical verification is the only verified-pass signal. Full turn observations
supply complete-cohort totals and per-pass ratios; incomplete observations retain
known lower bounds and separate coverage counts. Missing turns never enter the
complete denominator as zero. Retain sample counts for pass/failure turn coverage
and elapsed-time coverage. Distinguish mean verified-pass turns/time from total
observed effort per verified pass; document exact formulas. No profile rankings
or significance claims are produced.

## Operator boundary

`kiln-implementer metrics` remains preview-by-default with `--record true` for a
receipt. Add optional model turns to its usage input and prefer the retained
worker usage when no override is supplied. `metrics-report --from-ms N --to-ms N`
reads the existing database and emits the report; it does not dispatch, verify,
close work or publish. Subscription credentials are described using turns,
tokens, quota share when supplied and elapsed wall time, never invented dollars.

## Verification

Cover Completed/verified, Completed/rejected, Failed, Aborted/cancelled,
deadline, missing/incomplete usage, exact window edges, empty denominators,
profile/model grouping, partial coverage, overflow and restart/replay. Exercise
canonical rejection separately from verification infrastructure unknown outcomes.
Protect existing default-off metrics and exact-candidate authority tests.
