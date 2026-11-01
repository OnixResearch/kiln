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
- Only `phase = queued` runs are cancelled. Running, paused, unknown and
  awaiting-approval runs are untouched, so this change has no provider control
  effect.
- Cancellation uses the existing request-replay and durable queue path with a
  deterministic supersession request id. Failed cancellation is best-effort and
  never fails the new dispatch.
- The policy field is optional in Nickel and defaults to false.

## Risks

- Queue timing is inherently racy; a run that leaves the queued phase before
  supersession is not cancelled.
- Queued-only supersession does not implement running-run cancellation from the
  wider issue.
- The existing one-active-run-per-project queue remains the execution limiter.
