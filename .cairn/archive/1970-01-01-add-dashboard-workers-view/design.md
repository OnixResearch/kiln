## Context

The service admits project workflow bindings and serializes one nonterminal execution per project. Worker commands are one-shot reviewed processes; WorkflowPort exposes execution observations but no pool discovery or health operation. Existing Operations capacity counts projects rather than distinct execution targets. Retained state and profile identities must remain unchanged.

## Decisions

### Decision: Project existing facts, do not invent a worker registry

Group targets by provider_profile_id, workflow socket_path and platform. Derive a Kiln-owned BLAKE3 worker-target identifier from a versioned tuple; never expose socket paths, local repository paths, user IDs, credentials or worker argv. One target may serve several projects; configured/enrolled admission slots are not physical machine or CPU counts. Slot occupancy uses the existing queue predicate: a nonterminal run outside queued occupies its project slot, including ambiguous executions. Unknown executions never free capacity. Existing global failed-storage admission returns 503 rather than an inventory or usable free slots.

### Decision: Bounded immutable API

Authenticated GET /api/workers accepts no query or body and returns schema kiln.web-workers.v1, snapshot version, bounded=true, storage_failed, summary and workers. Viewer/operator may read; Worker role remains denied by existing worker-scope admission. Successful reads perform no provider operations, writes, clock reads or new persistence; denials retain existing attributable audit behavior. Inventory is bounded by max_projects/max_runs. Sorting is deterministic. Existing canonical service/worker profiles, DurableState and failed-storage admission are unchanged.

Each worker contains id, provider_profile_id, platform, liveness="unknown", status (slots_available, busy, unknown, not_enrolled), capacity {configured_slots,enrolled_slots,active,free_slots,queued}, projects [{id,label,enrolled,active_run_ids,queued}], and latest_run (null or {id,project_id,phase,outcome,sequence,error_code,created_at_ms,started_at_ms,completed_at_ms}). Summary contains targets, configured_slots, enrolled_slots, active, free_slots, queued. Status precedence is not_enrolled without enrolled projects, slots_available with free slots, unknown when a capacity-holding execution is ambiguous, otherwise busy. Latest execution is the greatest (ordinal,id) with phase != queued and either an observation, an error code or a nonterminal outcome; queued-only requests, including cancelled-before-dispatch requests, never replace execution evidence. Historical observation fields and timing are unknown when absent. Liveness always remains unknown because no health protocol exists.

### Decision: Integrate the existing interactive workspace

Add Workers navigation and a dedicated panel using the existing visual components, renderEndpoint generation guards, URL route handling and private-view reset. Client-side worker_q and worker_status filter the bounded inventory; worker selects one exact target independently of filters. A missing selection is not silently replaced. Show free configured slots separately from a liveness-unknown boundary; list assigned projects and link current/latest exact runs. Refresh reloads current facts. Search, selection, reload and browser history work on desktop/mobile. Logout and late responses cannot repopulate private data.

### Decision: Separate implementation owners

Backend owns engine/workers.rs, engine.rs module declaration, dashboard GET routing and engine worker regressions. UI owns web.html/web.js/web.css. Parent owns lifecycle, documentation, integration and real HTTPS/browser verification. All contributors skip checks mid-flight; the parent runs focused and required checks once integrated.

## Risks / Trade-offs

- A configured target is not a discovered physical worker; idle admission slots do not establish online status or successful dispatch. State labels and the claim boundary must keep this distinction visible.
- A provider profile may be reused across several distinct endpoints/platforms; grouping must include the full target binding while redacting paths. A shared target does not imply global placement or a provider concurrency guarantee.
- Preserve the previously recorded package-test and unrelated traceability failures. Source/UI work does not waive those holds or authorize production activation.
