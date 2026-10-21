# Tasks

## Phase 1: Scope and admission

- [x] [serial] Inspect the Kiln source shape, largest files, source adapter bounds, discovery selector and package build prerequisites; record the base commit and the fork/authority boundary. r[kiln.self_ci.authority] r[kiln.self_ci.source]
- [x] [serial] Admit proposal, design and tasks before implementation. r[kiln.self_ci.authority]

## Phase 2: Self-CI implementation

- [x] [serial] Add a typed self-CI profile with the base commit, source bounds, selector, selected task, product path and failure marker, with positive and negative fixtures. r[kiln.self_ci.authority] r[kiln.self_ci.source]
- [x] [serial] Clone the exact Kiln base commit, create the dedicated fork, and raise only the typed source bounds needed for the full tree. r[kiln.self_ci.source]
- [x] [serial] Add discovery selector/task hooks and select `packages.x86_64-linux` / `kiln` through the existing adapters. r[kiln.self_ci.build]
- [x] [serial] Admit flake package aliases that share one derivation in pure task mapping, retaining duplicate-attribute and dependency-cycle rejection. r[kiln.nix_eval_jobs.task_mapping] r[kiln.self_ci.build]
- [x] [serial] Keep the native host serving after a per-connection disconnect under the request bound, with a regression test. r[kiln.aspen_host.transport] r[kiln.self_ci.recovery]
- [x] [serial] Replace the synthetic case source with real remote Kiln commits and a tailored product/root case loop; preserve broker, controller, worker, publisher/observer and status rails. r[kiln.self_ci.build] r[kiln.self_ci.recovery]

## Phase 3: Self-CI proof

- [x] [serial] Exercise a real Kiln package success with the `bin/kiln` product, exact roots and remote status readback; retain the host-kill and remote-restart recovery. r[kiln.self_ci.build] r[kiln.self_ci.recovery]
- [x] [serial] Exercise a source-controlled Rust syntax failure with the genuine Nix diagnostic, duplicate delivery and unavailable-provider uncertainty. r[kiln.self_ci.build] r[kiln.self_ci.recovery]
- [x] [serial] Retain base/fork/source-shape/bounds, real revisions, ingress, journals, products/diagnostics, status, retirement and rejected attempts. r[kiln.self_ci.evidence] r[kiln.self_ci.claims]

## Phase 4: Delivery

- [x] [serial] Run focused tests and every required repository gate; synchronize and archive only after the self-CI prerequisites and acceptance criteria actually pass. r[kiln.self_ci.evidence]
