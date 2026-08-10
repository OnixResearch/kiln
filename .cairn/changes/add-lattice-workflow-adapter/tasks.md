## Phase 1: Contract and profile

- [ ] [serial] Pin the Lattice revision, licenses, workflow schema, observation schema, graph cohort, and supported platforms. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [ ] [depends:lattice-contract-pin] Add typed Nickel capabilities and bounds. r[kiln.lattice.profile]
- [ ] [parallel] Add profile fixtures for valid and invalid graph, control, effect, and limit combinations. r[kiln.lattice.conformance]

## Phase 2: Pure mapping

- [ ] [serial] Implement pure workflow request lowering and identity linkage. r[kiln.lattice.request]
- [ ] [depends:lattice-request-core] Implement pure lifecycle, completion, log, and reconciliation admission. r[kiln.lattice.observation] r[kiln.lattice.reconciliation]
- [ ] [parallel] Add positive and negative pure-core tests. r[kiln.lattice.conformance]

## Phase 3: Shell

- [ ] [depends:lattice-profile] Add `kiln-adapter-lattice` outside core dependency graphs. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [ ] [depends:lattice-observation-core] Implement bounded dispatch, observe, cancel, pause, resume, log, and reconcile operations. r[kiln.lattice.request] r[kiln.lattice.observation] r[kiln.lattice.reconciliation]
- [ ] [parallel] Add runtime independence and provider-unavailable tests. r[kiln.lattice.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add pinned producer and independent consumer fixtures. r[kiln.lattice.conformance]
- [ ] [serial] Document setup, identity, controls, logs, recovery, and non-claims. r[kiln.lattice.profile] r[kiln.lattice.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.lattice.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.lattice.conformance] r[kiln.lattice.nonclaims]
