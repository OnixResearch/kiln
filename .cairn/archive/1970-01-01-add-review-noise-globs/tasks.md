## Phase 1: Profile and admission

- [x] [serial] Add `review.noise` to the Rust profile with named limits and `kiln_core::paths` validation, the display identity, and the Nickel contract with positive and negative fixtures. r[kiln.review.noise] r[kiln.review.noise.scenario.invalid] r[kiln.review.noise.identity]

## Phase 2: Display

- [x] [serial] Annotate exact comparisons with matching rule ids in `code_review/noise.rs`, publish the identity, and render collapsed, display-derived pre-viewed noise files with a hidden count, rule ids and one-click expand in the console. r[kiln.review.noise.display] r[kiln.review.noise.scenario.fixture]

## Phase 3: Verification

- [x] [serial] Prove ownership over a noise path still blocks submit, legacy profiles are unchanged, run focused Rust, Nickel and console tests, a disposable-instance browser check, and document the display-only boundary in `docs/operator.md`. r[kiln.review.noise.authority] r[kiln.review.noise.scenario.ownership] r[kiln.review.noise.scenario.fixture]
