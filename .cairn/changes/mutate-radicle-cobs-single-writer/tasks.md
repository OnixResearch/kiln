## Phase 1: Profile and scope

- [x] [serial] Add a typed Nickel collaboration profile with SecretSpec declaration fields, RID, type, and target allowlists, single-writer policy, named deadlines, bounds, and non-claims. r[kiln.radicle.mutation_scope] r[kiln.radicle.shell_mutation] r[kiln.radicle.mutation_bounds]
- [x] [parallel] Add positive profile fixtures and negative missing-field, zero-deadline, unknown-provider, undeclared-target, and foreign-type fixtures. r[kiln.radicle.mutation_conformance]

## Phase 2: Pure decisions

- [x] [depends:mutation-profile] Add pure declared-target admission, single-writer head comparison under named deadlines, and phase outcome classification. r[kiln.radicle.mutation_scope] r[kiln.radicle.mutation_bounds]
- [x] [parallel] Add positive and negative core tests for substitution, moved heads, foreign heads, replayed operation identities, and timeout versus unknown classification. r[kiln.radicle.shell_mutation] r[kiln.radicle.mutation_conformance]

## Phase 3: Concrete transport

- [x] [depends:mutation-pure-decisions] Implement the concrete `radicle-cob` mutation transport over the pinned cohort with reload-compare-sign ordering and bounded phases. r[kiln.radicle.shell_mutation] r[kiln.radicle.mutation_bounds]
- [x] [parallel] Prove the collaboration profile cannot create or close patches, rewrite history, or reach undeclared targets. r[kiln.radicle.mutation_scope]

## Phase 4: Fixtures and evidence

- [x] [depends:mutation-transport] Add native positive and negative repository fixtures created by the pinned cohort, including repeat dispatch, foreign heads, missing signer, oversized payloads, timeout, and reconciliation. r[kiln.radicle.mutation_conformance]
- [x] [parallel] Add runtime-independence checks that keep Radicle crates outside core and standalone builds. r[kiln.radicle.mutation_conformance]
- [x] [parallel] Document authority flow, SecretSpec setup, recovery, limits, and non-claims. r[kiln.radicle.shell_mutation] r[kiln.radicle.mutation_scope]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and focused Nix gates. r[kiln.radicle.mutation_conformance]
- [x] [serial] Record evidence, sync accepted specs, archive the change, and rerun validation. r[kiln.radicle.mutation_conformance]
