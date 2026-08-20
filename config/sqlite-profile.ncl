let Contracts = import "contracts.ncl" in
let BusyTimeoutMillis = 250 in
let MaxEventsPerAppend = 64 in
let MaxReplayRecords = 65536 in
let MaxOutboxRecords = 65536 in
let MaxEventBytes = 1048576 in
let MaxSnapshotBytes = 16777216 in
let MaxBackupBytes = 1073741824 in
let MaxMigrationSteps = 16 in
let MaxRetentionDeletes = 4096 in
({
  database_path = "var/lib/kiln/events.db",
  open_mode = "read_write_create",
  rusqlite_version = "0.40.2",
  libsqlite3_sys_version = "0.38.2",
  sqlite_runtime_version = "3.53.2",
  license = "MIT",
  schema_version = 1,
  transaction_kind = "immediate",
  journal_mode = "WAL",
  synchronous = "FULL",
  backup_required = true,
  destructive_repair = false,
  bounds = {
    busy_timeout_millis = BusyTimeoutMillis,
    max_events_per_append = MaxEventsPerAppend,
    max_replay_records = MaxReplayRecords,
    max_outbox_records = MaxOutboxRecords,
    max_event_bytes = MaxEventBytes,
    max_snapshot_bytes = MaxSnapshotBytes,
    max_backup_bytes = MaxBackupBytes,
    max_migration_steps = MaxMigrationSteps,
    max_retention_deletes = MaxRetentionDeletes,
  },
  migration_edges = [{ from = 0, to = 1 }],
  non_claims = [
    "successful commit does not prove storage hardware durability",
    "backup integrity does not prove disaster recovery readiness",
    "outbox leases do not prove exactly-once provider execution",
  ],
}) | Contracts.SQLiteProfile
