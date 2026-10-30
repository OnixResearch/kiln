## Phase 1: Core

- [x] [serial] Add `kiln_core::trigger_filter` with bounded filters, per-kind admission and deterministic evaluation over `kiln_core::paths::matches` and `monorepo::ChangedFiles`. r[kiln.triggers.filter] r[kiln.triggers.filter.scenario.invalid] r[kiln.triggers.evaluate] r[kiln.triggers.evaluate.scenario.match] r[kiln.triggers.evaluate.scenario.unknown]
- [x] [serial] Add the review policy `optional_ci` rule and keep filtered required checks unmet otherwise. r[kiln.triggers.required] r[kiln.triggers.required.scenario.unmet] r[kiln.triggers.required.scenario.optional]

## Phase 2: Shells

- [x] [serial] Hosted pipelines: identity-preserving trigger wire format, trigger selection, execution run/skip records, and Actions filter migration. r[kiln.triggers.record] r[kiln.triggers.migrate] r[kiln.triggers.migrate.scenario.round_trip] r[kiln.triggers.replay]
- [x] [serial] Web automation and GitHub adapter: filter fields, pull request action admission, shared changed files, filtered and matched source records. r[kiln.triggers.record] r[kiln.triggers.replay]
- [x] [serial] Nickel contracts and fixtures for hosted pipelines, web automation and review policy; operator and hosted CI documentation. r[kiln.triggers.filter]

## Phase 3: Verification

- [x] [serial] Run the focused tests, Nickel fixtures and replay checks, and record evidence. r[kiln.triggers.filter] r[kiln.triggers.evaluate] r[kiln.triggers.record] r[kiln.triggers.required] r[kiln.triggers.migrate] r[kiln.triggers.replay]
