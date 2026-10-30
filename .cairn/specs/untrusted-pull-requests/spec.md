# Untrusted Pull Requests Specification

## Purpose

Defines the `untrusted-pull-requests` capability.

## Requirements

### Requirement: Reviewed untrusted pull request policy

r[kiln.untrusted.policy]
The GitHub repository policy MUST expose `untrusted_pull_requests` with values `deny` and `approve`, defaulting to `deny`. With `deny`, fork pull requests MUST be refused exactly as before. Existing profiles, fixtures, journals and run records MUST replay and serialize byte-identically. The retired `allow_fork_pull_requests` field MUST remain readable and MUST be rejected when `true`. Profiles without an approval surface MUST admit only `deny`.

#### Scenario: Deny is byte-identical

r[kiln.untrusted.policy.scenario.deny]
- GIVEN the existing deny automation fixture and a signed fork pull request event
- WHEN the profile is exported, admitted and re-serialized, and the event is admitted
- THEN the bytes MUST equal the retained baseline, AND the event MUST be refused with `github_fork_denied`

### Requirement: Untrusted classification

r[kiln.untrusted.classification]
With `approve`, a pull request MUST be untrusted when its head repository differs from the policy repository (`fork`), or when its author lacks push access (`author_without_push_access`). Push access MUST mean an `author_association` of `OWNER`, `MEMBER` or `COLLABORATOR`; any other or absent association MUST be untrusted. Same-repository pull requests from authors with push access MUST behave exactly as today.

### Requirement: Durable blocked run bound to the exact commit

r[kiln.untrusted.blocked]
An untrusted pull request that passes trigger selection MUST create a durable run in the `awaiting_approval` phase, bound to its exact source commit. That run MUST NOT dispatch before approval. A new commit on the same pull request MUST create a new blocked run and MUST supersede older unapproved runs of that pull request.

#### Scenario: New commit returns to blocked

r[kiln.untrusted.blocked.scenario.new_commit]
- GIVEN an untrusted pull request whose run for commit A awaits approval
- WHEN a signed synchronize event for commit B is admitted
- THEN a run for B MUST await approval, the run for A MUST be superseded and never dispatch, AND an approval naming A MUST be rejected

### Requirement: Untrusted run approval

r[kiln.untrusted.approval]
`ApproveUntrustedRun { actor, run, source_commit }` MUST be accepted only from an interactive operator principal. It MUST bind the run's exact source commit. On success it MUST record the approver and commit, and make the run eligible for dispatch. Viewer, worker and automation principals MUST be rejected. A stale or malformed commit, a superseded run, or a run not awaiting approval MUST be rejected without changing state.

#### Scenario: Unauthorized approval

r[kiln.untrusted.approval.scenario.unauthorized]
- GIVEN a run awaiting approval
- WHEN a viewer or an automation principal submits an approval for its exact commit
- THEN the approval MUST be rejected with 403 (the shell's read-only rule answers a viewer with `viewer_read_only` before any approval decision; an automation principal gets `untrusted_approval_unauthorized`), AND the run MUST stay in `awaiting_approval`

### Requirement: Untrusted execution constraints

r[kiln.untrusted.execution]
Approved untrusted runs and all their retries MUST dispatch only to execution targets declared untrusted-capable (per-job microVM). They MUST carry no secret environment, no workload token (`KILN_WORKLOAD_TOKEN`) and no cache restore or publication, with no per-run override. Undeclared targets MUST be refused before any external effect.

#### Scenario: Retry stays untrusted

r[kiln.untrusted.execution.scenario.retry]
- GIVEN an approved untrusted run with secrets, a workload audience and cache configured for its project
- WHEN the run and an exact retry are prepared for execution
- THEN neither MUST receive secrets, `KILN_WORKLOAD_TOKEN` or a cache receipt, AND a non-untrusted-capable runner MUST be refused

### Requirement: Review independence

r[kiln.untrusted.review]
Approving an untrusted run MUST NOT satisfy, change or substitute for review approvals, ownership, veto clearing or queue admission. An untrusted result MUST satisfy required CI only for its exact approved commit, and only when the reviewed trigger policy sets `untrusted_results_satisfy_required_ci`.

#### Scenario: Checklist never flips on approval

r[kiln.untrusted.review.scenario.checklist]
- GIVEN a change whose checklist `approvals` and `ci` items are unmet, and an untrusted CI run for its commit
- WHEN that run is approved and succeeds under a policy that does not count untrusted results
- THEN no checklist item MUST change from unmet to met

### Requirement: Untrusted run display

r[kiln.untrusted.console]
Run detail and the change-review checklist MUST show an untrusted run's state (`awaiting_approval`, `approved`, `superseded`), reason, approver, approved commit and whether it counts for required CI. Approval MUST go through the existing confirmation dialog and be offered only to operators. Operator and hosted CI documentation MUST describe the policy and the untrusted-runner requirement.
