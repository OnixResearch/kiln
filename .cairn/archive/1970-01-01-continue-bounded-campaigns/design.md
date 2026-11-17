## Context

`kiln_implementer::campaign::drive_bounded` runs at most `maximum_operations` intent-before-effect operations for one expected campaign version and stops on an unresolved effect, a decision without an effect, a blocked, replan, decision-request or complete decision, or its operation bound. The native shell `kiln-web-service drive-implementation-campaign` wraps it after role, ledger and context preflight. onix-core runs that command as an operator-started oneshot template unit per expected version. Campaign state lives in the `implementation_campaign` table with version compare-and-swap. Campaign operator commands (`start`, `inspect`, `cancel`, `continue`, `reconcile`, `issue_decision`, `resolve_decision`) mutate the same store.

## Decisions

### Decision: a bounded step, not a resident loop

**Choice:** `continue-implementation-campaign` performs at most `max_batches` decisions and driver batches, then exits with the next step: drive again, wait until a time or a hint, or stop. A supervisor (timer, path unit or explicit start, owned by #49) invokes it.

**Rationale:** the supervisor already owns process lifetime, restart and resource limits. A one-shot step cannot spin, and every invocation rereads canonical state.

### Decision: a pure decision over a durable continuation record

**Choice:** `ContinuationRecord` holds coalesced wake demand (a kind set and a monotonic `demand_sequence`), the last batch outcome, time and store version, consecutive failures, the next eligible time and a batch count. `decide` is pure:

1. Stop at completion, or at cancellation once no effect is unresolved.
2. Stop when consecutive failures reach the budget.
3. Wait until the backoff deadline.
4. Wait for an operator while paused with no unresolved effect.
5. Drive again after an operation-limit batch.
6. Drive when unconsumed demand exists.
7. Retry after a failed batch once its backoff has elapsed.
8. Drive on the first step, or when the store version moved since the last batch.
9. Drive when the rescan interval has elapsed since the last batch.
10. Otherwise wait until the rescan deadline.

An unresolved effect is not itself a reason to drive. While paused or cancelled it still reconciles on the triggers in 5 to 9, because the driver selects reconciliation before any admission. `apply_outcome` consumes demand only up to the sequence read before the batch, resets failures except after a failed batch, and computes exponential backoff with saturating arithmetic. An `operator` hint clears the failure count and backoff deadline.

**Rationale:**
- Hints are rereads, never effects, so duplicates and out-of-order hints coalesce.
- The sequence keeps a hint recorded mid-batch.
- Version changes catch operator commands and owner observations without hints.
- The rescan bounds a lost notification.
- Pausing stops new admission but not reconciliation.
- An unresolved effect waits for its owner's hint, a version change or the rescan, so owner read-back does not busy-loop.
- A failed batch retries at its backoff deadline even without demand.

### Decision: classify batches from typed driver results

**Choice:** `classify_batch` maps a driver result to one outcome:

| Outcome | When |
| --- | --- |
| `stale_version` | the error starts with the shared `STALE_CAMPAIGN_VERSION` prefix used by store and driver |
| `failed` | any other driver error |
| `admission_refused` | the run carries a claim refusal |
| `complete` | the campaign is complete |
| `cancelled` | the campaign is cancelled with nothing to reconcile |
| `awaiting_owner` | an effect stays unresolved |
| `operation_limit` | the batch used every operation and the next decision is an owner effect |
| `paused` | admission is paused |
| `needs_replan` | the decision is replan |
| `waiting` | otherwise |

Only `failed` counts against the budget.

**Rationale:** a refused claim or a waiting campaign is not a failure and must not burn retries. A stale version needs a reread, not backoff.

### Decision: canonical claim refusal is a capacity wait

**Choice:** `CampaignDriveShell::claim` returns `CampaignClaim::Bound(version)` or `CampaignClaim::Refused(reason)`. Exact canonical admission now returns `AdmissionResult::WriterBusy` for another active or unreconciled repository writer instead of an error, and `claim_campaign_work` surfaces it as `CampaignClaimOutcome::WriterBusy`. The native shell maps `NotWinner` and `WriterBusy` to `Refused`; lineage, policy and currency refusals stay errors and count as failures. `drive_bounded` stops before any intent and reports the refusal on the run.

**Rationale:** canonical admission remains the final owner. The campaign learns that capacity is unavailable without an error, and a `capacity` hint or rescan resumes it.

### Decision: one record table in the campaign database; operator commands hint

**Choice:** `implementation_campaign_continuation(campaign_id, version, bytes)` sits beside `implementation_campaign`. `record_wake` requires an existing campaign and merges hints in an immediate transaction. `record_batch` merges an outcome into the current record in one immediate transaction. Consumption is bounded by the demand sequence read before the batch, so a hint recorded during the batch stays pending; a compare-and-swap would instead discard the outcome whenever a hint arrived. `resolve_decision` records a `review` hint and `continue` records an `operator` hint in the same transaction as their canonical transitions, so a committed transition never loses its hint.

**Rationale:** there is no second queue and no progress copy, and restart keeps demand, backoff, failures and pending effects. Driver CAS on the campaign row still prevents duplicate claims, dispatch, publication or integration.

### Decision: policy in the drive profile with named defaults

**Choice:** an optional `continuation` block holds `rescan_interval_ms`, `retry_base_ms`, `retry_max_ms`, `max_consecutive_failures` and `max_batches`. Defaults are 300,000 ms, 30,000 ms, 3,600,000 ms, 8 failures and 4 batches. Validation bounds each field, requires `retry_base_ms <= retry_max_ms`, and is mirrored in the Nickel contract.

**Rationale:** existing profiles, including onix-core's generated ones, stay valid.

## Risks / Trade-offs

- Owner processes do not yet emit hints. Until they call the wake command, verification and source progress resume within one rescan interval.
- Two simultaneous invocations can both reach the driver. Campaign CAS keeps effects single; both outcomes merge into the record in order. Supervisors should still run one instance per campaign.
- Scripted owners prove ordering, coalescing, persistence and backoff. Native workers, models and remote effects stay unqualified here.
