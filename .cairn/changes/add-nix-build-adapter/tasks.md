# Tasks: Exact Nix Build Provider

## Phase 1: Cohort and request

- [x] [serial] Add the pinned Nix build profile, executable and derivation identity checks, runtime binding, limits, and non-claims. r[kiln.nix_build.cohort] r[kiln.nix_build.nonclaims]
- [x] [serial] Add pure request lowering and plan binding for the exact provider, plan, policy, platform, products, and `.drv`. r[kiln.nix_build.request]

## Phase 2: Execution and completion

- [x] [serial] Add the bounded runtime invocation with cleared environment, fixed `HOME`/`NIX_CONFIG`, no builders or substituters, and process-group teardown. r[kiln.nix_build.execution]
- [x] [serial] Parse the native Nix JSON report, admit required products, bind product and receipt identities, and map every outcome. r[kiln.nix_build.completion]
- [x] [serial] Add bounded stderr log identity and chunked log reads. r[kiln.nix_build.logs]
- [x] [serial] Add duplicate-dispatch, conflict, observe, reconcile, restore, and unknown-bound behavior. r[kiln.nix_build.reconciliation]

## Phase 3: Integration and fixtures

- [x] [serial] Add positive and negative fixture tests for success, missing output, malformed report, nonzero exit, timeout, cancellation, unknown, drift, bounds, logs, and restore. r[kiln.nix_build.completion] r[kiln.nix_build.logs] r[kiln.nix_build.reconciliation]
- [x] [serial] Add a real local Nix build test behind `nix-live` and run it with the pinned executable. r[kiln.nix_build.execution]
- [x] [serial] Add the typed Nickel profile, fixture, documentation, workspace member, and `nix-build-adapter-fixtures` Nix check. r[kiln.nix_build.cohort]

## Phase 4: Verification

- [x] [serial] Run the fixture tests, real local Nix test, strict Clippy, formatting, and the `nix-build-adapter-fixtures` Nix check. r[kiln.nix_build.execution] r[kiln.nix_build.completion]
- [x] [serial] Record exact commands, outcomes, and non-claims in `evidence/verification.md`. r[kiln.nix_build.nonclaims]
