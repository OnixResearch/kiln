# SQLite adapter validation

Date: 2026-08-20

## Baseline

- `nix develop path:$PWD -c cargo test --workspace` passed before implementation.

## Focused checks

- `cargo test -p kiln-adapter-sqlite` passed with nine positive and negative tests.
- Focused Clippy passed with `-D warnings`.
- `nickel export config/sqlite-profile.ncl` passed.
- Unsafe and zero-bound SQLite profiles failed as required.

## Checked behavior

- Events, stream revisions, and outbox records commit in one immediate transaction.
- Interrupted transactions expose neither event rows nor outbox rows.
- Replay validates sequence, schema, encoded bytes, and BLAKE3 event identities.
- Durable leases survive restart, and expired leases require reconciliation.
- Retention requires an exact snapshot and preserves the committed stream revision.
- Explicit migration accepts only the admitted `0 -> 1` edge.
- Lock, corruption, snapshot, backup, migration, and bound fixtures pass.

## Pinned cohort

- `rusqlite 0.40.2` with `backup` and `bundled` features.
- `libsqlite3-sys 0.38.2`.
- Bundled SQLite `3.53.2`.
- License: `MIT`.

## Repository checks

- Workspace tests, Clippy, `no_std`, and all Nickel fixtures passed.
- Cairn validation passed after spec sync.
- Tracey coverage passed at `351/351` referenced requirements.
- `nix flake check path:$PWD -L` passed all checks.

## Non-claims

These checks prove bounded SQLite transaction and recovery observations. They do not prove hardware durability, filesystem correctness, disaster recovery, or exactly-once execution.
