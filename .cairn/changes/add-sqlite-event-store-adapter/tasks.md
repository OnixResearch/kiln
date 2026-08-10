## Phase 1: Storage contract

- [ ] [serial] Select and pin the SQLite library cohort, features, licenses, platforms, and supported runtime settings. r[kiln.sqlite.profile] r[kiln.sqlite.nonclaims]
- [ ] [depends:sqlite-cohort] Define typed Nickel database, transaction, durability, bound, backup, and recovery policy. r[kiln.sqlite.profile]
- [ ] [parallel] Add positive policy fixtures and negative unsafe, missing, contradictory, and zero-bound fixtures. r[kiln.sqlite.conformance]

## Phase 2: Pure storage core

- [ ] [serial] Implement pure append-batch, revision, identity, and event-outbox linkage admission. r[kiln.sqlite.atomic_append]
- [ ] [depends:sqlite-append-core] Implement pure replay, lease, retention, and migration decisions. r[kiln.sqlite.replay] r[kiln.sqlite.leases] r[kiln.sqlite.migrations]
- [ ] [parallel] Add model tests for success, stale revisions, duplicates, gaps, expired leases, and invalid migrations. r[kiln.sqlite.conformance]

## Phase 3: SQLite shell

- [ ] [depends:sqlite-profile] Add `kiln-adapter-sqlite` outside core dependency graphs. r[kiln.sqlite.profile] r[kiln.sqlite.nonclaims]
- [ ] [depends:sqlite-storage-core] Implement schema creation, transactions, reads, leases, checkpoints, migrations, backup, and recovery. r[kiln.sqlite.atomic_append] r[kiln.sqlite.replay] r[kiln.sqlite.leases] r[kiln.sqlite.migrations]
- [ ] [parallel] Add deterministic temporary-database and fault fixtures. r[kiln.sqlite.conformance]

## Phase 4: Completion

- [ ] [serial] Document operation, backup, migration, damage response, and non-claims. r[kiln.sqlite.profile] r[kiln.sqlite.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.sqlite.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.sqlite.conformance] r[kiln.sqlite.nonclaims]
