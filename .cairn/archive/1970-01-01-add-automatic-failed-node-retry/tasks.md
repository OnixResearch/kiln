## Phase 1: Policy and state

- [x] [serial] Add the bounded per-project `retry_failed` option with Nickel and profile admission. r[kiln.web.retry_failed]
- [x] [serial] Store the automatic retry ordinal on the child run and keep the parent link. r[kiln.web.retry_failed]

## Phase 2: Automatic retry

- [x] [serial] Issue the existing failed-node rerun on the exact source with a deterministic request identity and the retained untrusted gate. r[kiln.web.retry_failed.scenario.pass]
- [x] [serial] Stop at the reviewed bound and create nothing for disabled or refused cases. r[kiln.web.retry_failed.scenario.bound] r[kiln.web.retry_failed.scenario.refused] r[kiln.web.retry_failed.scenario.disabled]

## Phase 3: Surface and verification

- [x] [serial] Show direct retry children and the failed-then-passed label without rewriting the parent. r[kiln.web.retry_failed.scenario.pass]
- [x] [serial] Document the option and run engine, UI-asset, Nickel and workspace checks. r[kiln.web.retry_failed]
