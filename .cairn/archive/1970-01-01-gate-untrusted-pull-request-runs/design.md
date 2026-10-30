## Context

- **GitHub event admission:** `kiln_adapter_github::events::admit_event` authenticates the webhook, then decodes. It compares the pull request head repository id with the policy repository id and returns `github_fork_denied` unless `EventPolicy.allow_fork_pull_requests` is true. Every Nickel contract pins that field to `false`, and web automation admission (`AutomationProfile::admit`) rejects `true`. The webhook payload's `pull_request.author_association` is not decoded today.
- **Web automation intake:** `engine/automation.rs::admit_automation_trigger` selects registrations (head branch, base branch, siblings; #13 adds filters there), checks that the local reference is at the event revision, reserves `SourceRecord`s and calls `POST /api/runs` as an automation principal with `Role::Operator`. `engine/queue.rs::enqueue` creates a `RunRecord` in phase `queued`. `dispatch_queued` dispatches queued runs through the project's `WorkflowPort`. `rerun` creates exact, latest or failed-node retries with `parent_run_id`.
- **Web execution:** a project's `workflow` names one execution target. Workers bind to the active run (`/api/worker/lookup`), restore and publish cache through `POST /api/runs/{id}/cache/{restore|publish}`, and run steps in bubblewrap, which is defense in depth for trusted CI only.
- **Hosted execution:** `kiln-hosted::execution::run_pipeline` resolves a `WorkloadPolicy` per organization, project and runner. That policy delivers secrets and `KILN_WORKLOAD_TOKEN`, and disables cache restore and publication (`disabled_secret_grants`). Firecracker runners exist (`Backend::Firecracker`, `kiln-microvm`).
- **Review:** `kiln_core::change_review` evaluates the `ci` checklist item from `CiFact`s exported by `engine/change_review.rs`. Approvals come only from eligible reviewer votes.
- **Roles:** web principals are `Viewer`, `Operator` and `Worker`. Automation and webhook principals (`github:*`, `automation:*`) are also `Operator` internally.

## Goals and Success Contract

With `deny`, which is the default, nothing changes: fork pull requests are refused with the same error, and existing profiles, fixtures, journals and run records serialize byte-identically.

With `approve`, an untrusted pull request never executes before an interactive operator approves its exact commit. A new commit always needs a new approval. An approved untrusted run, and every retry of it, receives no secret, no workload token and no cache. It runs only on an execution target declared untrusted-capable. Approving never changes a review approval, ownership, veto or queue fact. An untrusted result satisfies required CI only for its exact approved commit, and only when policy allows it.

False completion includes:
- a fork run dispatched before approval;
- an approval reused across commits;
- a retry that regains secrets or cache;
- automation or viewer principals able to approve;
- a checklist item flipping to met because of an approval;
- changed bytes for existing deny profiles.

## Decisions

### Decision: Pure core in `kiln_core::untrusted`

**Choice:** a `no_std` module owns the policy enum `UntrustedPullRequests { Deny, Approve }`, the reasons `UntrustedReason { Fork, AuthorWithoutPushAccess }` and `classify(policy, facts) -> TrustDecision { Trusted, Refused, Untrusted(reason) }`. It also owns the durable `UntrustedGate` (reason, pull request, head repository, exact `source_commit`, `satisfies_required_ci`, optional `UntrustedApproval`, optional `superseded_by`) and its derived `UntrustedState { AwaitingApproval, Approved, Superseded }`.

The approval command is `ApproveUntrustedRun { actor: Approver { id, label, authority }, run, source_commit }`, with `decide_approval`. Supporting functions:
- `supersede` handles a newer commit on the same pull request;
- `execution_grants(untrusted)` returns `ExecutionGrants`, which are all false for untrusted runs;
- `UntrustedGate::satisfies_required_ci(revision)` decides required-CI eligibility.

Commit ids are 40 or 64 lowercase hex characters (SHA-1 or SHA-256, for Tangled knots).

**Rationale:** the shells (web service, hosted, Tangled) share one decision. Tests exercise it without services.

### Decision: Policy wire format keeps deny byte-identical

**Choice:** `EventPolicy` gains `untrusted_pull_requests`, which is serde-default `deny` and omitted when `deny`. The retired `allow_fork_pull_requests` keeps its exact serialization so existing JSON replays. `admit_event_policy` now rejects `true`, which every contract already forbade. Nickel accepts `untrusted_pull_requests | optional` with values `"deny"` or `"approve"`, and keeps `allow_fork_pull_requests` pinned to `false`.

The standalone `kiln.github-ci.v1` contract and adapter profile admission accept only `deny`, because that service has no durable approval surface. `TriggerProfile` gains `untrusted_results_satisfy_required_ci` (default `false`, omitted when false), which is valid only with `approve`.

**Rationale:** existing profiles and fixtures export and replay unchanged. A test compares the deny fixture's JSON with a retained golden copy.

### Decision: Classification from signed webhook facts

**Choice:** `admit_event` decodes `pull_request.author_association` as optional. With `deny`, forks are refused exactly as today and same-repository pull requests are trusted, ignoring the association. With `approve`, a fork is `Fork`. A same-repository pull request whose association is not `OWNER`, `MEMBER` or `COLLABORATOR`, or is absent, is `AuthorWithoutPushAccess`. `VerifiedEvent.untrusted: Option<UntrustedReason>` is omitted when trusted. The candidate identity is unchanged, because the selector already binds the head repository.

**Rationale:** the webhook is authenticated, so its association is the only push-access evidence available without a provider call. Absence fails closed.

### Decision: Durable blocked run in the web service

**Choice:** after trigger selection (and #13 filters), an untrusted event's reserved sources dispatch `POST /api/runs` as before. The engine passes the pending gate through `automation_untrusted`, like `automation_affected`. `enqueue` stores `RunRecord.untrusted: Option<UntrustedGate>`, which is omitted when absent, and sets phase `awaiting_approval`. `dispatch_queued` only sees `queued`, so nothing executes.

Enqueueing a new gate supersedes every older unapproved gate with the same pull request and head repository but a different commit. Those runs complete as `Cancelled` with error `untrusted_superseded`, so a new commit returns the pull request to blocked. Cancelling a blocked run uses the existing queued-cancel path.

**Rationale:** the run exists, is visible, is bound to its exact source, and survives restart, because it is part of `DurableState`.

### Decision: Approval endpoint and approver role

**Choice:** `POST /api/runs/{id}/approve` takes `{request_id, source_commit}`. The shell maps the principal to `ApproverAuthority`:
- `Operator` only for an interactive operator, meaning the operator bearer token or a tailnet binding with role operator;
- `Automation` for `github:*` and `automation:*` principals;
- `Viewer` and `Worker` as is.

The shell's role rules refuse viewers (`viewer_read_only`) and workers (`worker_scope`) before any approval decision. For the remaining principals, `decide_approval` rejects with these codes:
- `untrusted_approval_unauthorized` (403);
- `untrusted_approval_stale_commit`, `untrusted_run_superseded`, `untrusted_run_not_awaiting_approval` (409);
- `untrusted_runner_unavailable` (409), when the project is not declared untrusted-capable.

On success, the shell records the approval, moves the run to `queued`, persists, audits and dispatches. Request ids replay through the existing request journal.

**Rationale:** an explicit role and rejection set. Approval never touches code-review state.

### Decision: Untrusted execution constraints

**Choice:**
- **Web service:**
  - `ProjectProfile.untrusted_capable` (default false, omitted when false) declares that the project's execution target runs each job in a microVM without credentials or cache. `dispatch_queued` refuses untrusted runs on undeclared projects.
  - The worker lookup binding carries `untrusted: true`. Cache restore and publish return 403 `untrusted_run_cache_denied`.
  - The worker refuses an untrusted binding unless its profile declares `untrusted_capable`, and never restores or publishes cache for one.
  - Rerun copies the gate, approval included, because the commit is exact. `latest` reruns of untrusted runs are rejected.
- **Hosted:**
  - `PipelineRequest.untrusted: Option<UntrustedProvenance>` is omitted when absent.
  - Every job runner must be `Backend::Firecracker { untrusted_capable: true }`, checked before any effect.
  - The pure `job_grants` drops the workload policy (no secrets, no `KILN_WORKLOAD_TOKEN`) and disables cache with outcome `disabled_untrusted`, so no restore or publish receipt exists.
  - Each retry attempt is admitted the same way, and attempt replay never re-executes.

**Rationale:** this reuses the secret-bearing cache rule and fails before external effects.

### Decision: Review integration

**Choice:** `CiFact.untrusted: Option<UntrustedCiFact>` is omitted when absent. The core `ci` item counts an untrusted fact only when `counts_for_required_ci` holds and the approved commit equals the evaluated commit. The shell exports `success` honestly. Approvals, vetoes, ownership and queue admission never read untrusted state. The checklist `ci` item gains `untrusted_runs` (state, approver, approved commit, counts flag), and the CI evidence card shows the same facts.

**Rationale:** the policy lives in the pure evaluator, and the display reuses the existing `ci_projects` enrichment pattern.

## Risks / Trade-offs

- `MEMBER` association is treated as push access. It means organization membership, which usually but not always implies push. Operators who need stricter behaviour can rely on fork detection, or keep `deny`.
- `untrusted_capable` is an operator declaration. Kiln checks the backend kind (Firecracker) in hosted, but cannot verify the web execution target's isolation.
- The worker's own credential file must be outside the job's mounts; this is the existing worker contract.
- Superseded runs show as cancelled with `untrusted_superseded`, rather than as a new outcome kind.

## Verification Strategy

- Core unit tests: classification matrix, approval decisions (authorized, unauthorized, stale, superseded, duplicate), supersession, grants and required-CI eligibility, with positive and negative cases.
- Adapter tests: deny fork refusal unchanged, approve classification, missing association, `allow_fork_pull_requests: true` rejected, deny serialization byte-identical.
- Web-service engine tests: deny profile and golden JSON byte-identical; blocked until approval; new commit re-blocks and supersedes; unauthorized approval rejected; approved run and retry bind untrusted with cache denied; undeclared runner refused; checklist never flips; replay of state without new fields byte-identical.
- Hosted tests: runner admission, and `job_grants` showing no secrets, token or cache for untrusted original and retry attempts.
- Nickel: positive and negative fixtures, and a deny export compared with the baseline.
- Disposable-instance browser check of the blocked run and the approve dialog.
