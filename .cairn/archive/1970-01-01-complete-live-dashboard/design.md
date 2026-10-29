## Context

The live service has bounded operator/worker REST, original-profile-bound SQLite state, source-bound Lattice execution, retained logs/artifacts and one flat UI. The offline catalog already owns reusable application CSS/navigation conventions. Its read-only boundary remains separate. The log follower incorrectly gates its timer on a bearer rather than the admitted session.

## Decisions

### Decision: Split independent owners and preserve one integration owner

Parent owns shared dependencies/manifests, lifecycle, operator docs, integration checks and host deployment. Engine owner owns engine.rs and engine/{api,model,storage,logs,artifacts}.rs, profile.rs, config/web-service.ncl and engine-focused tests. Streaming owner owns worker.rs and worker/{client,process,source,tests}.rs. Reports owner owns new root reports.rs, new worker/reports.rs, worker/profile.rs, config/web-worker.ncl and new report tests; Streaming owner integrates report calls and updates its Step test literals. Automation owner owns new root automation.rs, engine/automation.rs, config/web-automation.ncl and its focused tests. Access owner owns auth.rs, server.rs, main.rs, new access.rs, config/web-access.ncl and access/server tests. UI owner owns web.html/web.js/web.css only, reusing the existing catalog.css route and classes. No siblings edit another owner's files without a message and agreed transfer. Every worker MUST skip build/test/lint/format mid-flight; parent runs checks after integration.

### Decision: Data and durable-state contract

ServiceProfile and original worker/source bindings remain authoritative. Additive reviewed fields MUST skip absent/default serialization so existing profile identities do not change; existing runs gain unknown rather than invented historical timestamps. Retain original IDs, blobs and canonical events. New records have host-supplied UTC epoch milliseconds, ordinal, parent_run_id/retry selection, graph edges, observed node timing and bounded report/audit records. RunView retains existing fields and adds created_at_ms, started_at_ms, completed_at_ms, duration_ms, parent_run_id, edges, node timing and report summaries. Every optional unknown fact is JSON null, not fabricated. Queued phase has no provider effect until a durable admitted dispatch; original prepared/unknown dispatch is reconciled, never retried as queued work. Failed-node retry must store its exact selected graph and validate recovery against that subset, with worker execution restricted to the selected admitted node set.

### Decision: API contract

Existing project enrollment, run start, live, control, logs and artifacts remain real. New endpoint owners MUST send exact payload schemas to UI/parent early and coordinate additions in this file by message, not overwrite it.

| Route | Contract / owner |
|---|---|
| GET /api/session | principal {id,label,role}, permissions {read,operate,configure}; Access identity + Engine integration |
| GET /api/overview | totals, projects with health/latest run, duration trend and recorded timing coverage; Engine |
| GET /api/projects/:id/refs | admitted source references and current exact revision; Engine |
| GET /api/runs | existing worker exact-source lookup unchanged; operator/viewer q, project_id, outcome, source_revision, limit,cursor,order query with runs,next_cursor,total,bounded; Engine |
| POST /api/runs | project_id,request_id plus optional admitted reference/source_revision; latest is explicit UI wording; Engine |
| POST /api/runs/:id/rerun | request_id, mode exact/latest/failed; new run and candidate, never alias prior attempt; Engine |
| GET /api/runs/:id | extended RunView including actual graph/node timing; Engine |
| GET /api/runs/:id/activity | bounded canonical events and attributable audit records; Engine |
| GET /api/runs/:id/evidence | source,pipeline,execution,artifact and recorded report linkage; Engine |
| GET /api/runs/:id/logs | existing byte cursor plus bounded literal search parameter q; Engine |
| GET /api/runs/:id/logs/download?node_id=&stream= | complete retained selected stream, explicit retained/truncated facts, safe filename; Engine |
| POST /api/runs/:id/reports | worker-only exact binding plus kind,name,body_base64,blake3; parse bounded real declared files with reports.rs; Engine/Worker |
| GET /api/runs/:id/reports | normalized tests/coverage/evidence with raw retained artifact/report identity; Engine/Worker |
| GET /api/runs/:id/compare?other= | actual outcome/duration/test/coverage comparison and repeated-test instability from admitted reports; Engine |
| GET /api/activity | bounded cross-run canonical and audit timeline; Engine |
| GET /api/operations | queue, capacity, provider observations, usage and warnings; Engine |
| GET /api/automation | admitted trigger/schedule/notification registration and durable enabled/status/delivery state; Automation |
| POST /api/automation | strict operator-only request_id,kind,id,enabled and bounded registered schedule/trigger/notification options; Automation |
| POST /api/triggers/:id | authenticated signed registered push/PR intake bound to exact repository/source, explicit replay prevention; Automation |

Access owner provides crate::access::Principal {id:String,label:String,role:Role} and Engine::api_as(method,target,body,&Principal) is authoritative HTTP entry. Engine::api(method,target,body,Role) remains the real embedding port with a stable system principal, not a bypass; it applies the same permission checks. Role gains Viewer. All writes deny Viewer before effects and every mutation/denial carries actor identity. Access owner MUST coordinate this signature with Engine owner.

Optional --access PATH and --automation PATH are explicit separately admitted runtime configuration, leaving canonical ServiceProfile identity unchanged. Existing check/serve --profile use their bounded default policy. No environment-driven hidden configuration. Access owner handles CLI/server option loading and coordinates AutomationProfile handoff. Engine owner provides configure_automation(profile) and calls its bounded tick after normal polling. Automation owner implements these methods in engine/automation.rs, adds typed AutomationState (serde default) for DurableState. Engine owner adds the module/field only; automation owner does not edit shared engine files.

### Decision: Reusable streaming and reports

Bounded Exec currently returns captures only after completion; its owner requires an exact published Git revision for adoption, and no remote publication is authorized. Keep its approved process supervisor unchanged. The worker therefore creates private bounded Unix-stream transports and invokes a fixed stream-child helper under Bounded Exec. The helper connects stdout/stderr to those owned transports and replaces itself with the reviewed command using exec: no second supervisor or child execution tree is introduced. Bounded Exec retains the actual command PID, process-group deadline, cancellation and teardown authority. Worker transport readers reuse the approved bounded-exec-core CaptureAccumulator; an owned bounded uploader sends retained chunks during execution. Helper arguments carry only owned socket paths and reviewed argv, never credentials. No shell tee or unbounded temporary output. Transport failure/overflow is explicit, success requires admitted ingress, and existing cancellation/timeout/failure precedence remains authoritative. Access owner adds the stream-child CLI dispatch supplied by Streaming owner.

Worker declarations add bounded optional report declarations (JUnit and Cobertura/LCOV coverage plus recorded gate/review/provenance JSON) with safe relative paths and exact node binding. reports.rs owns strict bounded parsing/normalization, not source of authority. Reject XML entities/DOCTYPE and malformed/oversized data, preserve interoperability formats, retain exact BLAKE3 source bytes and distinguish recorded reports from gate acceptance. Structured test failures never infer execution success.

### Decision: Access and automation authority

Tailnet browsing stays tokenless. A trusted proxy identity resolver uses actual tailscale WhoIs over a private local port/socket and an explicit registered user-to-role map. Backend accepts forwarded identity only with a separate private proxy credential and never from arbitrary unauthenticated headers. Local operator/worker credentials keep their roles; viewer credentials may be configured separately. All private values stay out of browser/store/URL/logs. Access owner owns a real proxy-auth helper entry point and repeatable host integration instructions.

Automation uses registered typed server-owned repositories/refs, signing-secret references and HTTPS notification destinations; dashboard selects/edits bounded registrations rather than arbitrary paths, commands or URLs. Reuse existing GitHub event admission and webhook notification mechanisms behind shell adapters. Schedules persist next-due and reservation identities before dispatch; notifications reserve delivery before network, preserve definite/unknown outcomes and do not automatically retry ambiguous acceptance. Operator may configure interval schedules with UTC next due; no implied cron/timezone feature. Parent provisions an actual local consumer configuration and verifies signed push/PR, schedule and HTTPS receiver behavior in disposable state without enabling unrequested recurring production effects.

## Risks / Trade-offs

This is a complete local CI workspace, not hosted accounts/billing, arbitrary source/command authority, automatic retention pruning, profile migration by deletion, per-node invented progress or remote publication. Runtime policy and access changes have their own exact recorded identities. Pure decisions receive explicit facts; clocks, network, filesystem and credentials stay in shells.

## Verification Strategy

Gate proposal/design/tasks before implementation. Each slice supplies paired behavior/error regression tests but skips running checks mid-flight. Parent runs focused/full required consumer checks once integrated, then actual native worker/service/browser paths: multi-chunk network log follow, pre-exit streaming, search/download byte equality, deep-link/reload/back and invalid IDs/cursors, graph/timing, exact/failed/latest reruns and restart queue identity, reports/coverage/comparison/malformed/stale inputs, evidence boundaries, signed trigger replay and due schedules, actual HTTPS notifications, viewer and operator and forged headers, usage/provider failures, desktop/mobile shell and disabled capabilities. Finally update owner docs/evidence and deploy without deleting history or changing unrelated tailnet routes.
