## Phase 1: Core

- [x] [serial] Add optional `board` configuration to the view payload with bounds, change-time status validation and old-view defaults. r[kiln.work.kanban.config] r[kiln.work.kanban.config.scenario.migration] r[kiln.work.kanban.config.scenario.invalid]
- [x] [serial] Add the `set_status` issue command with team workflow validation through the existing store path. r[kiln.work.kanban.move] r[kiln.work.kanban.move.scenario.stale] r[kiln.work.kanban.move.scenario.retry]
- [x] [serial] Add the pure board core: column derivation, Other lane, card bounds, completeness and filter semantics. r[kiln.work.kanban.columns] r[kiln.work.kanban.columns.scenario.unknown] r[kiln.work.kanban.projection]

## Phase 2: Shell and console

- [x] [serial] Serve the read-only `GET /api/work/board` projection. r[kiln.work.kanban.projection]
- [x] [serial] Add `work-board.js` pure helpers, render the Kanban board with Move dialog, drag-and-drop, retry and conflict resolution, and edit board columns in the view editor. r[kiln.work.kanban.interaction] r[kiln.work.kanban.columns] r[kiln.work.kanban.boundary]
- [x] [serial] Document board configuration, moves and the planning-status boundary. r[kiln.work.kanban.boundary]

## Phase 3: Verification

- [x] [serial] Run model, board and integration Rust tests, node tests and a disposable-instance browser smoke at 1440 and 320 px; retain evidence. r[kiln.work.kanban.config] r[kiln.work.kanban.columns] r[kiln.work.kanban.move] r[kiln.work.kanban.projection] r[kiln.work.kanban.interaction] r[kiln.work.kanban.boundary]

Evidence: `evidence/verification.json`.

Accepted on operator request 2026-10-30 after parent integration review; see `evidence/verification.json`.
