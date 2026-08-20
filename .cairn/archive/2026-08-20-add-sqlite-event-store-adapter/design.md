# Design: SQLite event-store adapter

## Context

Kiln requires atomic event and outbox persistence. The standalone shell demonstrates the transition but does not provide durable service storage.

SQLite supplies local transactions and recovery. Kiln still owns event schemas, revisions, effect identities, replay rules, and retention policy.

## Goal and Success Contract

The goal is a durable local `EventStorePort` with deterministic schema and recovery behavior.

Completion requires atomic append, optimistic revision checks, replay compatibility, leases, migrations, backup evidence, and crash fixtures.

False completion includes separate event and outbox commits, implicit migrations, false durability claims, swallowed corruption, or SQLite types in core crates.

## Decisions

### Decision: Keep storage plans pure

Pure functions validate append batches, expected revisions, event identities, outbox linkage, lease transitions, replay bounds, and migration steps.

The shell owns database connections, files, transactions, locking, synchronization calls, clocks, and error translation.

### Decision: Commit events and outbox together

One transaction writes the accepted event batch and every requested effect record. A failed transaction exposes neither side.

The append uses an expected stream revision. A mismatch fails without retrying against a newer state.

### Decision: Use explicit schema versions

Each database records one schema version and migration history. Migrations use deterministic ordered steps with preflight and postflight checks.

Unsupported or partially applied versions block service admission. Automatic destructive repair is not permitted.

### Decision: Preserve canonical bytes and identities

The store retains canonical encoded event bytes, event identities, sequence facts, and contract versions. Reads validate stored identity before replay.

Derived indexes and snapshots remain rebuildable. They cannot replace canonical events.

### Decision: Make durability policy explicit

Typed policy selects transaction mode, journal mode, synchronization level, busy behavior, size bounds, retention, checkpoint policy, and backup requirements.

Evidence reports the selected and observed settings. A commit does not prove persistence beyond the recorded SQLite and filesystem observations.

### Decision: Keep leases and reconciliation durable

Outbox claims record owner, lease state, lease bound, attempts, provider profile, and reconciliation state. The shell supplies time observations.

Expired or ambiguous leases enter reconciliation. They do not become undispatched work automatically.

### Decision: Fail closed on damage

Integrity errors, identity mismatches, unsupported versions, partial migrations, and missing canonical rows block service admission or the affected operation.

The adapter never invents missing events from projections or provider state.

## Configuration

Typed Nickel configuration defines the database location, open mode, schema cohort, transaction policy, durability settings, bounds, retention, backup, and recovery behavior.

## Validation

Model tests compare pure append, replay, lease, and migration decisions. Real database fixtures cover restart, crash, lock, corruption, backup, migration, and interrupted transaction cases.

## Claim Boundary

Passing checks establishes bounded SQLite transaction and recovery observations. It does not prove storage hardware durability, filesystem correctness, or exactly-once provider execution.
