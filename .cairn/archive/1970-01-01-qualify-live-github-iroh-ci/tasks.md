## Phase 1: Admission

- [x] [serial] Validate proposal, design, and tasks before implementation; select exact forge repository, isolated ref/ingress, separate hosts, real SecretSpec provider, and reviewed producer/runtime cohort. r[kiln.live_ci.authority]

## Phase 2: Composition

- [x] [serial] Provide the reviewed operator-input composition and repeatable live exercise using normal GitHub/Iroh binaries, exact-source helper, real Lattice, private credentials/state, and explicit enrollment. Preserve existing services and canonical CI authority. r[kiln.live_ci.composition] r[kiln.live_ci.authority]
- [x] [serial] Select the existing typed loopback listener on an isolated controller host; verify that its port is unused and preserve unrelated ingress handlers without changing the configuration contract. r[kiln.live_ci.authority]

## Phase 3: Live verification

- [x] [serial] Record a real GitHub delivery, cross-host compile/test success, canonical Success, and matching Published GitHub success status with exact source/workflow/execution identities. r[kiln.live_ci.outcomes]
- [x] [serial] Record a genuine failing test at another exact source revision, canonical DomainFailure, and matching Published GitHub failure status; verify durable duplicate suppression. r[kiln.live_ci.outcomes]
- [x] [serial] Interrupt and restart only the new controller/bridges while the worker job continues; recover the same execution without duplicate work and record its matching terminal GitHub status. r[kiln.live_ci.recovery]
- [x] [serial] Run required affected checks and native lifecycle gates; document exact repetition commands, credential/isolation limits, immediate result, durable contribution, and maintenance ownership. r[kiln.live_ci.authority]
