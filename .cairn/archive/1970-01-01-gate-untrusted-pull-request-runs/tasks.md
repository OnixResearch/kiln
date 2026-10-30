## Phase 1: Core

- [x] [serial] Add `kiln_core::untrusted` with policy, classification, gate, approval decision, supersession, execution grants and required-CI eligibility, plus positive and negative tests. r[kiln.untrusted.classification] r[kiln.untrusted.approval] r[kiln.untrusted.approval.scenario.unauthorized] r[kiln.untrusted.execution]
- [x] [serial] Add `CiFact.untrusted` and count untrusted CI only for its exact approved commit, under explicit policy. r[kiln.untrusted.review] r[kiln.untrusted.review.scenario.checklist]

## Phase 2: Shells

- [x] [serial] GitHub adapter: `untrusted_pull_requests` policy, retired-field admission, author association, classification, deny-only standalone profiles. r[kiln.untrusted.policy] r[kiln.untrusted.policy.scenario.deny] r[kiln.untrusted.classification]
- [x] [serial] Web service: trigger policy admission, durable blocked runs, supersession, approval endpoint, untrusted-capable dispatch gate, cache denial, worker binding and refusal, retry inheritance, CI facts. r[kiln.untrusted.blocked] r[kiln.untrusted.blocked.scenario.new_commit] r[kiln.untrusted.approval] r[kiln.untrusted.execution] r[kiln.untrusted.execution.scenario.retry] r[kiln.untrusted.review]
- [x] [serial] Hosted execution: Firecracker `untrusted_capable` declaration, `PipelineRequest.untrusted`, runner admission, credential and cache suppression. r[kiln.untrusted.execution] r[kiln.untrusted.execution.scenario.retry]
- [x] [serial] Console: run detail state, approver and approved commit, approve confirmation dialog, change-review checklist display. r[kiln.untrusted.console]
- [x] [serial] Nickel contracts and fixtures, `docs/operator.md` and `docs/hosted-ci.md`. r[kiln.untrusted.policy] r[kiln.untrusted.console]

## Phase 3: Verification

- [x] [serial] Run the focused tests, Nickel fixtures, byte-identity checks and a disposable-instance browser check, and record evidence. r[kiln.untrusted.policy] r[kiln.untrusted.classification] r[kiln.untrusted.blocked] r[kiln.untrusted.approval] r[kiln.untrusted.execution] r[kiln.untrusted.review] r[kiln.untrusted.console]
