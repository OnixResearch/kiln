# Tasks

## Phase 1: Stable case contract

- [x] [serial] Add stable case_id, recipe, BLAKE3 recipe_revision, source identity, presentation and interaction requirements to Example and /examples.json. r[kiln.web.exploration.cases]
- [x] [serial] Resolve four named cases against seeded runtime IDs and routes. r[kiln.web.exploration.cases]
- [x] [serial] Migrate index.js title/href/detail consumers to case-aware rendering. r[kiln.web.exploration.cases]
- [x] [serial] Migrate Bombadil admit_catalog to validate case/recipe fields while retaining title/href/detail. r[kiln.web.exploration.cases]

## Phase 2: Selection and capture

- [x] [serial] Add case selection/compare view with keyboard and narrow/dark support. r[kiln.web.exploration.cases]
- [x] [serial] Add bounded capture helper with screenshot, semantic observation and provenance manifest. r[kiln.web.exploration.cases]
- [x] [serial] Record Kiln-to-Rats migration cohort as blocked on #67/#68. r[kiln.web.exploration.cases]

## Phase 3: Verification

- [x] [serial] Parent runs cargo tests and clippy for explore example. r[kiln.web.exploration.cases]
- [x] [serial] Parent runs node test and Bombadil rail. r[kiln.web.exploration.cases]
