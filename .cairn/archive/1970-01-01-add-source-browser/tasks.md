## Phase 1: Reader

- [x] [serial] Add the typed source policy, exact Git/Jujutsu revision, tree and file reads, bounded Pijul program reads and paired rejection coverage. r[kiln.dashboard.source.read] r[kiln.dashboard.source.authority]

## Phase 2: View

- [x] [serial] Add the Source navigation view with source selection, revision paging, breadcrumb tree and bounded file rendering, restored from the URL. r[kiln.dashboard.source.view]

## Phase 3: Operation

- [x] [serial] Admit operator repositories for all three kinds, exercise installed reads, run required checks and retain bounded evidence with the operator procedure. r[kiln.dashboard.source.read] r[kiln.dashboard.source.authority] r[kiln.dashboard.source.view]

Observed live reads, rejections and read-only checks are retained in
`evidence/verification.json`.
