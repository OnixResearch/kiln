# Tasks: Exact Nix Build Provider

## Phase 1: Cohort and exact request

- [x] [serial] Pin the repo Nix 2.34.8 `nix` and `nix-store` executables and add exact expected output names and store paths. r[kiln.nix_build.cohort] r[kiln.nix_build.request]
- [x] [serial] Add the profile, runtime binding, limits, non-claims, and flat config loader. r[kiln.nix_build.cohort] r[kiln.nix_build.nonclaims]
- [x] [serial] Add pure request lowering and plan binding for the exact provider, plan, policy, platform, products, and `.drv`. r[kiln.nix_build.request]

## Phase 2: Rooted execution and completion

- [x] [serial] Switch to `nix-store --add-root … --indirect --realise` with cleared environment, fixed `HOME`/`NIX_CONFIG`, no builders or substituters, and process-group teardown. r[kiln.nix_build.execution]
- [x] [serial] Admit native realisation roots, exact targets, products, receipt identities, and every outcome. r[kiln.nix_build.completion]
- [x] [serial] Add effect-scoped GC-root retention and idempotent release. r[kiln.nix_build.retention]
- [x] [serial] Add bounded stderr log identity and chunked log reads. r[kiln.nix_build.logs]
- [x] [serial] Add duplicate-dispatch, conflict, observe, reconcile, restore, and unknown-bound behavior. r[kiln.nix_build.reconciliation]

## Phase 3: Composition and operator surface

- [x] [serial] Add the `kiln-nix-build` task-to-profile/plan mapping and real derivation hashing. r[kiln.nix_build.composition]
- [x] [serial] Add the `doctor`, `build`, and `from-task` CLI, including bounded log output and optional root release. r[kiln.nix_build.composition]
- [x] [serial] Add fixture and live discovery-shaped task builds and rooted release checks. r[kiln.nix_build.composition] r[kiln.nix_build.retention]

## Phase 4: Integration and fixtures

- [x] [serial] Add multi-output, output-path-mismatch, malformed-root, retention, cohort-drift, limit, log, and restore fixture tests. r[kiln.nix_build.completion] r[kiln.nix_build.logs] r[kiln.nix_build.reconciliation]
- [x] [serial] Add real local adapter and composition tests behind `nix-live` and a host live-build script. r[kiln.nix_build.execution] r[kiln.nix_build.composition]
- [x] [serial] Update the typed Nickel profile, documentation, workspace members, conformance boundaries, and Nix checks. r[kiln.nix_build.cohort] r[kiln.nix_build.composition]

## Phase 4b: State, preflight, and host policy

- [x] [serial] Add a bounded local-input preflight and a failure path before rooted execution. r[kiln.nix_build.preflight]
- [x] [serial] Add durable bound/task state with store/load/rehydrate, observe, reconcile, and idempotent release. r[kiln.nix_build.state]
- [x] [serial] Pin the sandbox, `NIX_CONF_DIR`, and no-experimental-features Nix policy. r[kiln.nix_build.execution]
- [x] [serial] Add real timeout, cancellation, and multi-output live cases. r[kiln.nix_build.execution] r[kiln.nix_build.completion]
- [x] [serial] Add a real `NixEvalAdapter` discovery-to-build live E2E and extend the host rail. r[kiln.nix_build.composition]

## Phase 5: Verification

- [x] [serial] Run both fixture suites, both live suites, strict Clippy, formatting, Nickel checks, and the Nix checks. r[kiln.nix_build.execution] r[kiln.nix_build.completion]
- [x] [serial] Run the CLI end-to-end with a discovery-shaped task, bounded log, and released roots. r[kiln.nix_build.composition]
- [x] [serial] Record exact commands, outcomes, and non-claims in `evidence/verification.md`. r[kiln.nix_build.nonclaims]
