## Why

Every Kiln GitHub intake path refuses fork pull requests by contract: `allow_fork_pull_requests` must be `false` in `config/web-automation.ncl`, `config/github-contracts.ncl` and `config/github-ci.ncl`. Outside contributors, including agents acting from their own accounts (#3), get no CI at all, so maintainers reproduce their changes by hand. `docs/operator.md` already says enabling forks needs a separately reviewed policy and stronger isolation, not a changed fixture. Tangled's model is the reference: such pull requests run only after a collaborator approves, and every approved run and its retries is untrusted, with no secrets, caches or ID token (issue #12).

## What Changes

- **Reviewed policy:** the fixed constant is replaced by `untrusted_pull_requests: deny | approve` on the GitHub repository policy. `deny` is the default and keeps today's refusal exactly. Existing profiles, fixtures, journals and run records replay byte-identically; the retired `allow_fork_pull_requests` field stays readable and must remain `false`.
- **Classification:** with `approve`, a pull request is untrusted when its head is a fork, or when its author lacks push access (GitHub `author_association` other than `OWNER`, `MEMBER` or `COLLABORATOR`, or absent). Same-repository branches from principals with push access behave exactly as today.
- **Blocked runs:** an untrusted pull request that passes trigger selection creates a run in a durable **Blocked: awaiting approval** state bound to the exact source commit. Nothing executes. A new commit on the same pull request creates a new blocked run and supersedes the older unapproved one.
- **Approval command:** `ApproveUntrustedRun { actor, run, source_commit }`, served as `POST /api/runs/{run}/approve`. Only an interactive operator principal may approve. Viewer, worker and automation principals are rejected, and so is a stale commit.
- **Untrusted execution:** approved runs and their retries dispatch only to execution targets declared untrusted-capable (per-job microVM), never with secret references, workload token audience, or cache restore/publication. This holds in the web service (dispatch gate, run binding, cache endpoints, worker) and in hosted execution (`PipelineRequest.untrusted`, Firecracker `untrusted_capable` runners, no workload policy, cache disabled). There is no per-run override.
- **Review:** approving a run is not a code-review approval. An untrusted result satisfies required CI only for its exact approved commit, and only when the trigger's reviewed policy sets `untrusted_results_satisfy_required_ci`.
- **Console:** run detail and the change-review checklist show the blocked state, reason, approver and approved commit. Operators approve through the existing confirmation dialog.
- **Configuration and docs:** Nickel contracts with positive and negative fixtures; `docs/operator.md` and `docs/hosted-ci.md` describe the policy and the untrusted-runner requirement.

## Impact

- **Consumers:** maintainers who want CI for outside and agent contributors (#3); the Tangled adapter (#14), which maps `approvePipeline` onto the approval command; trigger filters (#13), which still apply before the gate; debug holds (#15), which are never offered for untrusted runs.
- **Owners:** Kiln maintainers own `kiln_core::untrusted` and the GitHub adapter classification. Web-service maintainers own intake, approval, dispatch, console and review integration. Hosted maintainers own runner declaration and credential/cache suppression.
- **Affected areas:** `crates/kiln-core` (`untrusted`, `change_review::CiFact`), `crates/kiln-adapter-github` (events), `crates/kiln-web-service` (automation, engine, worker, profile, code review, console), `crates/kiln-hosted` (execution), Nickel contracts and fixtures, `docs/operator.md`, `docs/hosted-ci.md`.

## Out of Scope

- An approval surface in the standalone `kiln-github` adapter service and the hosted GitHub connection. Their reviewed profiles accept only `deny`, so they refuse fork pull requests as today. Untrusted gating lives in the web automation intake, which owns durable runs and the operator console.
- Proving push access through a GitHub API call. Classification uses the signed webhook's `author_association` and fails closed when it is absent.
- Hostile-isolation qualification of the microVM backend. `untrusted_capable` is an operator declaration of microVM placement without credentials or cache; independent adversarial qualification remains separate evidence.
- Rejecting a run as a separate state. Operators cancel a blocked run instead.
