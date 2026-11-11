## Phase 1: Ownership

- [x] [serial] Add session/fence/host/generation owner tokens and bounded admission. r[kiln.hosted.retained.lifecycle]
- [x] [serial] Refuse stale owners and generations before every lifecycle command. r[kiln.hosted.retained.lifecycle.scenario.stale]

## Phase 2: Retained lifecycle

- [x] [serial] Add restore, confinement-gated pause, resume, migration, migrated-away, reconciliation and retirement states. r[kiln.hosted.retained.lifecycle]
- [x] [serial] Keep migration success separate from target placement and capacity release. r[kiln.hosted.retained.lifecycle.scenario.migration]
- [x] [serial] Retain capacity for reconciliation-required sessions and fail closed on missing confinement. r[kiln.hosted.retained.lifecycle.scenario.confinement]
