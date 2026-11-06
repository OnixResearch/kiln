## Phase 1: Core

- [x] [serial] Add the job profile, plan, wire actions, reducer and admission bounds with positive and negative unit tests. r[kiln.radicle.job] r[kiln.radicle.job.identity] r[kiln.radicle.job.bounds]
- [x] [serial] Add deterministic run identity, replay classification and conflict rejection. r[kiln.radicle.job.replay] r[kiln.radicle.job.scenario.replay]

## Phase 2: Shell

- [x] [serial] Add bounded discovery, create-once, signer admission and namespace-only append through the pinned multi-writer graph. r[kiln.radicle.job.identity] r[kiln.radicle.job.multi_writer] r[kiln.radicle.job.authority]
- [x] [serial] Reconcile without redispatch and reject commit mismatch, ambiguous jobs and unauthorized states. r[kiln.radicle.job.scenario.mismatch] r[kiln.radicle.job.scenario.extend]

## Phase 3: Verification

- [x] [serial] Add native fresh, replay, start/finish, foreign-job extension, divergent multi-head and malformed-action tests. r[kiln.radicle.job.multi_writer] r[kiln.radicle.job.bounds]
- [x] [serial] Add a COB browser fixture showing a Kiln-published job and update operator documentation. r[kiln.radicle.job.console]
- [x] [serial] Run focused tests, Nickel checks, workspace tests, Clippy, rustfmt and Cairn gates, then record evidence. r[kiln.radicle.job]
