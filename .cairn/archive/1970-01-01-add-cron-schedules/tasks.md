## Phase 1: Core

- [x] [serial] Add `kiln_web_service::schedule` with the pinned `jiff` bundled time-zone database: cron grammar, BLAKE3 `H` resolution, DST-safe occurrences, bounded minimum-interval review and the reservation decision. r[kiln.schedules.cron] r[kiln.schedules.hash] r[kiln.schedules.hash.scenario.restart] r[kiln.schedules.hash.scenario.identity] r[kiln.schedules.dst] r[kiln.schedules.dst.scenario.new_york] r[kiln.schedules.min_interval] r[kiln.schedules.min_interval.scenario.reject] r[kiln.schedules.no_catch_up]

## Phase 2: Shells

- [x] [serial] Web automation cron schedules: admission, configure, reservation with exact revision, runtime interval check and no catch-up. r[kiln.schedules.no_catch_up] r[kiln.schedules.no_catch_up.scenario.restart] r[kiln.schedules.replay]
- [x] [serial] Hosted pipeline schedule entries, schedule trigger, review check and durable reservation ledger. r[kiln.schedules.hosted] r[kiln.schedules.replay]
- [x] [serial] Nickel contracts and fixtures, operator and hosted CI documentation, and the `LICENSES.md` dependency record. r[kiln.schedules.cron]

## Phase 3: Verification

- [x] [serial] Run the focused tests and Nickel fixtures, and record evidence. r[kiln.schedules.cron] r[kiln.schedules.hash] r[kiln.schedules.dst] r[kiln.schedules.min_interval] r[kiln.schedules.no_catch_up] r[kiln.schedules.hosted] r[kiln.schedules.replay]
