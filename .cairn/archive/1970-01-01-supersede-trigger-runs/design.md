## Context

`RunRecord` now retains the requested symbolic reference separately from the
resolved source revision. Automation dispatch already reserves and starts the
new run durably before returning. Queued cancellation is an existing durable
state transition with request replay.

## Decisions

- Supersession is trigger-scoped and opt-in. A trigger with
  `supersede_queued = true` cancels older queued runs after its new run is
  created.
- Matching requires the same trigger registration, project and source scope, a
  different exact revision, and a non-new run id. A push source matches its
  requested reference and no pull-request number; a pull-request source matches
  its pull-request number. Manual and scheduled runs are not trigger origins
  and are never matched.
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

## Origin reconciliation

Trigger and schedule identifiers occupy separate admitted registration
namespaces and may have the same text. `requested_trigger` alone therefore
cannot establish a run's origin. Supersession candidates must be resolved from
retained automation source records with `kind = "trigger"`, the exact
registration identifier, and the linked Kiln run identifier before applying
the existing project/reference/pull-request/revision/phase filters.

Use the already persisted, validated source kind and run linkage. Preserve
scheduled metadata and existing serialized state; do not forbid valid shared
identifiers, add wire fields, encode string prefixes, or introduce compatibility
aliases. Missing provenance is ineligible for supersession. The existing
trigger-only dispatch guard remains unchanged, as do queued and exact-provider
control paths. Regressions must cover active and queued scheduled runs after
restart and the reverse schedule-arrival direction, with positive trigger,
manual and pull-request scope checks retained.


## Durable start-request recovery

A crash may persist the run and its Start request reservation before the
automation source's `run_id` is saved. A missing link is therefore resolved
only for a source still in `reserved` status and through its retained exact
automation request: require Start kind and the fingerprint of the expected
typed `/api/runs` request, then use its resource as the existing run. Ignored,
filtered and skipped sources never reserved a run and cannot establish its
origin, even if a manual request copies their expected fields. Share one
private source-to-StartRequest
constructor with normal dispatch so optional fields and request identity cannot
drift. Resolution performs no replay, API call, provider effect or state change;
the already linked normal path remains allocation-free.

Candidates must have a strictly smaller immutable run ordinal than the run
initiating supersession. Persistence increments the canonical state version
before later starts, while replay reuses the original run and ordinal. This
prevents delayed replay of an older source from cancelling its newer successor.
Exercise the persisted missing-link window, a mismatched request fingerprint,
and the later replay as well as normal origin exclusions.

