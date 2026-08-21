## Phase 1: Contract pin

- [x] [serial] Pin the Mantle revision, licenses, request schema, result schema, receipt schema, digest cohort, and supported platforms. r[kiln.mantle.profile] r[kiln.mantle.nonclaims]
- [x] [depends:mantle-contract-pin] Add a typed Nickel profile with capabilities and named bounds. r[kiln.mantle.profile]
- [x] [parallel] Add positive profile fixtures and negative version, capability, bound, and digest fixtures. r[kiln.mantle.conformance]

## Phase 2: Pure adapter core

- [x] [serial] Implement pure request planning and canonical Kiln linkage. r[kiln.mantle.request]
- [x] [depends:mantle-request-core] Implement pure product, metric, receipt, and outcome admission. r[kiln.mantle.completion]
- [x] [parallel] Add tests for cache confusion, builder confusion, stale completion, and missing products. r[kiln.mantle.conformance]

## Phase 3: Provider shell

- [x] [depends:mantle-profile] Add `kiln-adapter-mantle` outside core dependency graphs. r[kiln.mantle.profile] r[kiln.mantle.nonclaims]
- [x] [depends:mantle-completion-core] Implement bounded dispatch, observation, cancellation, log access, and reconciliation. r[kiln.mantle.request] r[kiln.mantle.completion] r[kiln.mantle.reconciliation]
- [x] [parallel] Add runtime independence and unavailable-provider tests. r[kiln.mantle.nonclaims]

## Phase 4: Completion evidence

- [x] [serial] Add pinned producer and independent consumer fixtures. r[kiln.mantle.conformance]
- [x] [serial] Document setup, identities, cache facts, recovery, and non-claims. r[kiln.mantle.profile] r[kiln.mantle.nonclaims]
- [x] [serial] Run formatting, workspace tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.mantle.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.mantle.conformance] r[kiln.mantle.nonclaims]
