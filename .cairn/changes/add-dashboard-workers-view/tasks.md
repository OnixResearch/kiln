## Phase 1: Implementation

- [x] [parallel] Implement bounded pure target grouping and occupancy projection, authenticated read-only API and paired availability/redaction/authorization/unknown-state regressions. r[kiln.dashboard.workers.inventory] r[kiln.dashboard.workers.capacity]
- [x] [parallel] Add the Workers page, filters, exact URL-restored selection, current-run links, refresh and session-safe desktop/mobile interactions. r[kiln.dashboard.workers.view]

## Phase 2: Verification

- [x] [serial] Exercise the real authenticated service and browser for worker inventory, state transitions, navigation, unavailable states and privacy; run focused and required repository checks and retain bounded evidence. r[kiln.dashboard.workers.inventory] r[kiln.dashboard.workers.capacity] r[kiln.dashboard.workers.view]
