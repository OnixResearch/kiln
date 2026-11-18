## Phase 1: Observation contract

- [x] [serial] Add complete/absent/incomplete model-request counts and retain exact worker terminal/profile/model plus controller verifier outcomes without changing authority. r[kiln.implementation.outcome_metrics]
- [x] [serial] Normalize admitted bounded worker observations with explicit unsupported telemetry and no event-count substitution. r[kiln.implementation.outcome_metrics.scenario.usage]
- [x] [serial] Retain outcomes across SQLite restart and reject stale/conflicting observations. r[kiln.implementation.outcome_metrics.scenario.binding]

## Phase 2: Operator report

- [x] [serial] Add deterministic windowed outcome and efficiency reporting with raw counts, coverage, failure split and profile/model groups. r[kiln.implementation.outcome_metrics.scenario.report]
- [x] [serial] Expose the opt-in CLI and document subscription cost units and non-authority. r[kiln.implementation.outcome_metrics.scenario.authority]
- [x] [serial] Run focused core, shell, adapter, canonical verifier and CLI checks; retain bounded evidence. r[kiln.implementation.outcome_metrics.scenario.report]
