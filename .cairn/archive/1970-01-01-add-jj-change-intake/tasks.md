## Phase 1: Core and adapter

- [x] [serial] Add `valid_jujutsu_change_id`, `JUJUTSU_CHANGE_ID_LENGTH` and the serde-default `conflicted_commit` fact with admission to `kiln_core::source`, with positive and negative tests. r[kiln.review.jj_intake] r[kiln.review.jj_intake.admission]
- [x] [serial] Add the read-only jj CLI observer to `kiln-adapter-jj` (op-head read, store check, bounded `log`/`evolog` with fixed templates, pure parsers, `JujutsuFacts` construction), with real-jj fixture tests including a byte-identical read-only check. r[kiln.review.jj_intake.admission] r[kiln.review.jj_intake.read_only] r[kiln.review.jj_intake.admission.scenario.divergent] r[kiln.review.jj_intake.admission.scenario.conflicted] r[kiln.review.jj_intake.read_only.scenario.unchanged]

## Phase 2: Review shell

- [x] [serial] Add the `intake.jujutsu` profile, the `jj_contribute` action, the internal member action, the pure planner, the orchestrator, stack bookkeeping, observations and the git change-id header path in `code_review/jj_intake.rs`, with small hooks in `commands.rs`, `model.rs`, `intake.rs` and `code_review.rs`. r[kiln.review.jj_intake] r[kiln.review.jj_intake.stack] r[kiln.review.jj_intake.header] r[kiln.review.jj_intake.observations]
- [x] [serial] Add web-service tests over real jj fixtures: rewritten change, divergent/conflicted refusals, duplicates, 4-change stack, partial edit, reorder, removed flag, header stack and byte-identical legacy replay. r[kiln.review.jj_intake.scenario.rewritten] r[kiln.review.jj_intake.scenario.replay] r[kiln.review.jj_intake.stack.scenario.create] r[kiln.review.jj_intake.stack.scenario.partial] r[kiln.review.jj_intake.stack.scenario.reorder] r[kiln.review.jj_intake.stack.scenario.removed] r[kiln.review.jj_intake.header.scenario.stack] r[kiln.review.jj_intake.observations.scenario.compatibility]

## Phase 3: Surfaces

- [x] [serial] Add the CLI options, the console contribution dialog and the evolog section on change detail. r[kiln.review.jj_intake.evolog] r[kiln.review.jj_intake.surface]
- [x] [serial] Add the Nickel contract with positive and negative fixtures and document the boundary in `docs/operator.md`. r[kiln.review.jj_intake.surface]

## Phase 4: Verification

- [x] [serial] Run focused fmt/clippy/tests, Nickel checks, node console tests and a disposable-instance smoke; record evidence. r[kiln.review.jj_intake] r[kiln.review.jj_intake.admission] r[kiln.review.jj_intake.read_only] r[kiln.review.jj_intake.observations] r[kiln.review.jj_intake.stack] r[kiln.review.jj_intake.header] r[kiln.review.jj_intake.evolog] r[kiln.review.jj_intake.surface]
