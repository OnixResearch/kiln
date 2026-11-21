## Phase 1: Change package and contract

- [x] [serial] Write the proposal, design, tasks and delta specs for the preserve-route realize cohort and capability admission. r[kiln.convergence.realize]
- [x] [serial] Add the typed `config/mantle-foreign-realize-profile.ncl` profile with positive and negative Nickel contract fixtures and run the profile contract check. r[kiln.mantle.foreign_realize.profile]

## Phase 2: Core and adapter

- [x] [serial] Declare `Operation::Realize` and admit `PreserveCacheOnly` only for a profile that declares it, with a stable profile-mismatch refusal for `BuildFromPlan`. r[kiln.convergence.verify]
- [x] [serial] Implement the realize profile admission, plan-keyed input bindings, identity verification and the exact bounded dispatch in `kiln-adapter-mantle`. r[kiln.mantle.foreign_realize.dispatch]
- [x] [serial] Implement closed-world realization-receipt completion, producer digest recomputation, product-to-root mapping and distinct failure classes with recorded-observation replay. r[kiln.mantle.foreign_realize.completion]
- [x] [serial] Implement recovery and idempotency: repeated dispatch returns the recorded observation and reconciliation never re-realizes. r[kiln.mantle.foreign_realize.recovery]

## Phase 3: Evidence and qualification

- [x] [serial] Retain the pilot realization receipt as a fixture and add positive and negative adapter tests for completion, identities, roots, products, bounds, failure classes and replay. r[kiln.mantle.foreign_realize.completion]
- [x] [serial] Run the single live pilot qualification through the public adapter API and record the commands, exit codes, receipt and comparison. r[kiln.convergence.realize]
- [x] [serial] Update `docs/mantle-foreign-realize.md`, `docs/README.md` and `docs/cairn-convergence.md`, then run the focused tests, clippy, fmt, Nickel, Cairn validate and the three gates. r[kiln.mantle.foreign_realize.nonclaims]
