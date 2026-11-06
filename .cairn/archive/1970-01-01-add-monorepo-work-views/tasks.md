## Phase 1: Core types and filters

- [x] [serial] Add bounded `CiFacts`, `ci_project`/`ci_repository` filters, the planning-project set and `group_by = ci_project` to the pure work model and matching rules. r[kiln.work.monorepo.facts] r[kiln.work.monorepo.filter] r[kiln.work.monorepo.planning] r[kiln.work.monorepo.grouping]
- [x] [serial] Add `Board.lane_by = ci_project`, bounded lane projection, single-card assignment, Other lane and `move_columns`, preserving status boards and old bytes. r[kiln.work.monorepo.board] r[kiln.work.monorepo.compat]

## Phase 2: Shell and console

- [x] [serial] Expose derived `kiln.work.ci.v1` facts and CI-aware listing and board responses in the work API shell, and build the facts from the admitted profile and retained runs in the `/api/work` route. r[kiln.work.monorepo.facts] r[kiln.work.monorepo.filter]
- [x] [serial] Add console filters, multi-project selection, CI grouping, row chips, lane labels, route restoration and drag restrictions. r[kiln.work.monorepo.grouping] r[kiln.work.monorepo.board]

## Phase 3: Verification

- [x] [serial] Add positive and negative model, board, API and compatibility tests and run focused Cargo, Clippy, rustfmt and JavaScript checks. r[kiln.work.monorepo.facts] r[kiln.work.monorepo.filter] r[kiln.work.monorepo.planning] r[kiln.work.monorepo.grouping] r[kiln.work.monorepo.board] r[kiln.work.monorepo.compat]
- [x] [serial] Exercise the real Work API and board with a monorepo profile, record evidence, update operator documentation and run Cairn validation. r[kiln.work.monorepo.facts] r[kiln.work.monorepo.board] r[kiln.work.monorepo.compat]
