# Add a SQLite event-store adapter

## Why

Kiln defines an `EventStorePort` and an in-memory reference journal. It has no durable local event-store adapter for service operation and crash recovery.

A SQLite adapter can provide one-file local durability without changing canonical event meaning.

## What Changes

- Add optional `kiln-adapter-sqlite` support for the `EventStorePort`.
- Store canonical events, stream revisions, outbox records, leases, snapshots, and migration state.
- Commit accepted events and requested outbox effects in one database transaction.
- Use pure functions for append admission, replay plans, lease transitions, and migration decisions.
- Reject stale expected revisions and duplicate event identities.
- Define explicit synchronization, WAL, busy, corruption, backup, and recovery policies.
- Add typed Nickel configuration with named bounds and durability choices.
- Add crash, lock, corruption, migration, backup, and replay fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted durable operations contract.

## Impact

- **Adapters**: adds `kiln-adapter-sqlite`.
- **Core**: remains independent from SQLite and filesystem types.
- **Configuration**: adds typed database, transaction, retention, and recovery policy.
- **Testing**: adds deterministic model tests and real temporary-database fixtures.
