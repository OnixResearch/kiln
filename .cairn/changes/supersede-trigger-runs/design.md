## Context

`RunRecord` now retains the requested symbolic reference separately from the
resolved source revision. Automation dispatch already reserves and starts the
new run durably before returning. Queued cancellation is an existing durable
state transition with request replay.

## Decisions

- Supersession is trigger-scoped and opt-in. A trigger with
  `supersede_queued = true` cancels older queued runs after its new run is
  created.
- Matching requires the same project, the same requested reference, a different
  exact revision, and a non-new run id. Runs with no recorded reference are not
  matched.
- Older queued runs complete through the durable queued-control path. Older
  active runs use the existing exact provider control path with the run's
  attempt, revision, sequence and execution, after a refresh. Awaiting-approval
  and terminal runs are untouched.
- A provider refusal, stale control or unsupported cancellation is best-effort
  and never fails the new dispatch.
- Cancellation uses the existing request-replay and durable queue path with a
  deterministic supersession request id. Failed cancellation is best-effort and
  never fails the new dispatch.
- The policy field is optional in Nickel and defaults to false.

## Risks

- Supersession is racy; a run that reaches a terminal state first is not
  cancelled.
- Provider cancellation may be refused or ambiguous; this behavior is bounded
  to the admitted provider-control result.
- The existing one-active-run-per-project queue remains the execution limiter.
