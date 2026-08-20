## Phase 1: Contract and profile

- [x] [serial] Pin the Lattice revision, licenses, workflow schema, observation schema, graph cohort, and supported platforms. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [x] [depends:lattice-contract-pin] Add typed Nickel capabilities and bounds. r[kiln.lattice.profile]
- [x] [parallel] Add profile fixtures for valid and invalid graph, control, effect, and limit combinations. r[kiln.lattice.conformance]

## Phase 2: Pure mapping

- [x] [serial] Implement pure workflow request lowering and identity linkage. r[kiln.lattice.request]
- [x] [depends:lattice-request-core] Implement pure lifecycle, completion, log, and reconciliation admission. r[kiln.lattice.observation] r[kiln.lattice.reconciliation]
- [x] [parallel] Add positive and negative pure-core tests. r[kiln.lattice.conformance]

## Phase 3: Shell

- [x] [depends:lattice-profile] Add `kiln-adapter-lattice` outside core dependency graphs. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [x] [depends:lattice-observation-core] Implement bounded dispatch, observe, cancel, pause, resume, log, and reconcile operations. r[kiln.lattice.request] r[kiln.lattice.observation] r[kiln.lattice.reconciliation]
- [x] [parallel] Add runtime independence and provider-unavailable tests. r[kiln.lattice.nonclaims]

## Phase 4: Evidence

- [x] [serial] Add pinned producer and independent consumer fixtures. r[kiln.lattice.conformance]
- [x] [serial] Document setup, identity, controls, logs, recovery, and non-claims. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.lattice.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.lattice.conformance] r[kiln.lattice.nonclaims]
