## Phase 1: Contracts and architecture

- [x] [serial] Validate architecture, requirements and native implementation gates. r[kiln.hosted.iroh]

## Phase 2: Hosted product behavior

- [x] [parallel] Implement authenticated Iroh customer API and local browser bridge with current organization/project authorization. r[kiln.hosted.iroh]
- [x] [parallel] Implement remote repository connection onboarding, generation-bound revocation and upstream exact-source CI integration. r[kiln.hosted.connections]
- [x] [parallel] Implement organization membership, invitations, scoped roles and revocation. r[kiln.hosted.accounts]
- [x] [parallel] Implement durable usage, exact prices, atomic spend admission, invoices and reconciliation. r[kiln.hosted.finance]
- [x] [parallel] Implement repository-owned pipelines, templates, parameters, matrices and fail-closed migration. r[kiln.hosted.pipelines]
- [x] [parallel] Implement isolated execution and bounded elastic fleet operations with actual resource and teardown observations. r[kiln.hosted.execution]
- [x] [parallel] Implement scoped secrets and short-lived job identity with expiration and revocation. r[kiln.hosted.secrets]
- [x] [parallel] Implement retention, backup/restore, incident controls and operational support observations. r[kiln.hosted.operations]

## Phase 3: Qualification

- [x] [serial] Exercise the integrated customer journey, positive/negative recovery and access cases, browser/Iroh surfaces and applicable repository gates; retain bounded evidence and update operator documentation. r[kiln.hosted.qualification]

Qualification evidence: [local runtime](evidence/local-runtime.json) and
[repository gates](evidence/verification.json). Repository traceability/release
readiness remains held by other active-change references; no archive, release,
production activation or external mutation is authorized by this task completion.
