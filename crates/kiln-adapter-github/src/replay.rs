use std::path::Path;
use std::time::Duration;

use kiln_core::types::ContentId;
use kiln_provider_api::PortResult;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::events::VerifiedEvent;
use crate::profile::DATABASE_BUSY_TIMEOUT_MS;
use crate::service::RunRecord;
use crate::{domain, infrastructure};

const DATABASE_VERSION: i64 = 1;
const DATABASE_SCHEMA: &str = "kiln.github-replay.v1";
const WAL_AUTOCHECKPOINT_PAGES: i64 = 100;

#[derive(Clone, Copy, Debug)]
pub struct StoreLimits {
    pub maximum_runs: usize,
    pub maximum_deliveries: usize,
    pub maximum_record_bytes: usize,
}

#[derive(Debug)]
pub struct Reservation {
    pub run_identity: ContentId,
    pub inserted: bool,
}

#[derive(Debug)]
pub struct SqliteStore {
    connection: Connection,
    profile_identity: ContentId,
    limits: StoreLimits,
}

impl SqliteStore {
    // r[impl kiln.github.replay]
    // r[impl kiln.github.shell]
    pub fn open(path: &Path, profile_identity: ContentId, limits: StoreLimits) -> PortResult<Self> {
        if !path.is_absolute()
            || limits.maximum_runs == 0
            || limits.maximum_deliveries < limits.maximum_runs
            || limits.maximum_record_bytes == 0
        {
            return Err(domain(
                "github_store_profile",
                "github.store",
                "GitHub storage requires an absolute path and positive bounded retention",
            ));
        }
        for value in [
            limits.maximum_runs,
            limits.maximum_deliveries,
            limits.maximum_record_bytes,
        ] {
            sqlite_count(value)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| storage_error("github_store_parent"))?;
        }
        let mut connection =
            Connection::open(path).map_err(|_| storage_error("github_store_open"))?;
        connection
            .busy_timeout(Duration::from_millis(DATABASE_BUSY_TIMEOUT_MS))
            .map_err(|_| storage_error("github_store_timeout"))?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| storage_error("github_store_version"))?;
        if version == 0 {
            let tables: i64 = connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(|_| storage_error("github_store_schema"))?;
            if tables != 0 {
                return Err(domain(
                    "github_store_unversioned",
                    "github.store",
                    "Existing unversioned database cannot be adopted as GitHub replay state",
                ));
            }
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage_error("github_store_transaction"))?;
            transaction.execute_batch(
                "CREATE TABLE github_meta (singleton INTEGER PRIMARY KEY CHECK(singleton=1), schema TEXT NOT NULL, profile_identity TEXT NOT NULL);
                 CREATE TABLE github_runs (run_identity TEXT PRIMARY KEY NOT NULL, immutable_identity TEXT NOT NULL, version INTEGER NOT NULL, phase TEXT NOT NULL, work INTEGER NOT NULL, record BLOB NOT NULL);
                 CREATE INDEX github_work ON github_runs(work, run_identity);
                 CREATE TABLE github_deliveries (delivery_id TEXT PRIMARY KEY NOT NULL, body_identity TEXT NOT NULL, run_identity TEXT NOT NULL REFERENCES github_runs(run_identity));
                 PRAGMA user_version=1;"
            ).map_err(|_| storage_error("github_store_schema"))?;
            transaction
                .execute(
                    "INSERT INTO github_meta VALUES (1, ?1, ?2)",
                    params![DATABASE_SCHEMA, profile_identity.as_str()],
                )
                .map_err(|_| storage_error("github_store_metadata"))?;
            transaction
                .commit()
                .map_err(|_| storage_error("github_store_commit"))?;
        } else if version != DATABASE_VERSION {
            return Err(domain(
                "github_store_version",
                "github.store",
                "GitHub replay database version is unsupported",
            ));
        }
        let metadata = connection
            .query_row(
                "SELECT schema, profile_identity FROM github_meta WHERE singleton=1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(|_| storage_error("github_store_metadata"))?;
        if metadata.0 != DATABASE_SCHEMA || metadata.1 != profile_identity.as_str() {
            return Err(domain(
                "github_store_profile_conflict",
                "github.store",
                "GitHub replay state belongs to a different schema or exact service profile",
            ));
        }
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| storage_error("github_store_journal"))?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| storage_error("github_store_sync"))?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| storage_error("github_store_foreign_keys"))?;
        connection
            .pragma_update(None, "wal_autocheckpoint", WAL_AUTOCHECKPOINT_PAGES)
            .map_err(|_| storage_error("github_store_checkpoint"))?;
        Ok(Self {
            connection,
            profile_identity,
            limits,
        })
    }

    // r[impl kiln.github.replay]
    pub fn reserve(
        &mut self,
        event: &VerifiedEvent,
        mut record: RunRecord,
    ) -> PortResult<Reservation> {
        record.restore_state()?;
        if record.profile_identity != self.profile_identity {
            return Err(domain(
                "github_store_profile_conflict",
                "github.store",
                "GitHub run profile differs from durable storage",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage_error("github_store_transaction"))?;
        let delivery = transaction
            .query_row(
                "SELECT body_identity, run_identity FROM github_deliveries WHERE delivery_id=?1",
                params![event.delivery_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|_| storage_error("github_delivery_read"))?;
        if let Some((body, run)) = delivery {
            if body != event.body_identity.as_str() || run != record.run_identity.as_str() {
                return Err(domain(
                    "github_delivery_conflict",
                    "github.delivery",
                    "GitHub delivery identity conflicts with its durable body or candidate",
                ));
            }
            transaction
                .commit()
                .map_err(|_| storage_error("github_store_commit"))?;
            return Ok(Reservation {
                run_identity: record.run_identity,
                inserted: false,
            });
        }
        let delivery_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM github_deliveries", [], |row| {
                row.get(0)
            })
            .map_err(|_| storage_error("github_delivery_count"))?;
        if delivery_count >= sqlite_count(self.limits.maximum_deliveries)? {
            return Err(retention_error());
        }
        let existing = transaction
            .query_row(
                "SELECT immutable_identity FROM github_runs WHERE run_identity=?1",
                params![record.run_identity.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| storage_error("github_run_read"))?;
        let inserted = match existing {
            Some(identity) => {
                if identity != record.immutable_identity.as_str() {
                    return Err(domain(
                        "github_candidate_conflict",
                        "github.run",
                        "GitHub candidate identity conflicts with the exact durable pipeline",
                    ));
                }
                false
            }
            None => {
                let count: i64 = transaction
                    .query_row("SELECT COUNT(*) FROM github_runs", [], |row| row.get(0))
                    .map_err(|_| storage_error("github_run_count"))?;
                if count >= sqlite_count(self.limits.maximum_runs)? {
                    return Err(retention_error());
                }
                let bytes = encode(&record, self.limits.maximum_record_bytes)?;
                transaction.execute("INSERT INTO github_runs (run_identity, immutable_identity, version, phase, work, record) VALUES (?1,?2,0,?3,?4,?5)", params![record.run_identity.as_str(), record.immutable_identity.as_str(), record.phase.as_str(), record.has_work(), bytes]).map_err(|_| storage_error("github_run_insert"))?;
                true
            }
        };
        transaction.execute("INSERT INTO github_deliveries (delivery_id,body_identity,run_identity) VALUES (?1,?2,?3)", params![event.delivery_id, event.body_identity.as_str(), record.run_identity.as_str()]).map_err(|_| storage_error("github_delivery_insert"))?;
        transaction
            .commit()
            .map_err(|_| storage_error("github_store_commit"))?;
        Ok(Reservation {
            run_identity: record.run_identity,
            inserted,
        })
    }

    pub fn load(&self, run: &ContentId) -> PortResult<Option<RunRecord>> {
        let row = self
            .connection
            .query_row(
                "SELECT version, record FROM github_runs WHERE run_identity=?1",
                params![run.as_str()],
                stored_row,
            )
            .optional()
            .map_err(|_| storage_error("github_run_read"))?;
        row.map(|(version, bytes)| {
            decode(
                version,
                &bytes,
                &self.profile_identity,
                self.limits.maximum_record_bytes,
            )
        })
        .transpose()
    }

    pub fn next_work(&self, after: Option<&str>) -> PortResult<Option<RunRecord>> {
        let mut row = self.connection.query_row("SELECT version,record FROM github_runs WHERE work=1 AND run_identity>?1 ORDER BY run_identity LIMIT 1", params![after.unwrap_or("")], stored_row).optional().map_err(|_| storage_error("github_work_read"))?;
        if row.is_none() && after.is_some() {
            row = self.connection.query_row("SELECT version,record FROM github_runs WHERE work=1 ORDER BY run_identity LIMIT 1", [], stored_row).optional().map_err(|_| storage_error("github_work_read"))?;
        }
        row.map(|(version, bytes)| {
            decode(
                version,
                &bytes,
                &self.profile_identity,
                self.limits.maximum_record_bytes,
            )
        })
        .transpose()
    }

    pub fn active_count(&self) -> PortResult<usize> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM github_runs WHERE phase NOT IN ('queued','terminal')",
                [],
                |row| row.get(0),
            )
            .map_err(|_| storage_error("github_active_count"))?;
        usize::try_from(count).map_err(|_| storage_error("github_active_count"))
    }

    pub fn save(&mut self, record: &mut RunRecord) -> PortResult<()> {
        record.restore_state()?;
        if record.profile_identity != self.profile_identity {
            return Err(domain(
                "github_store_profile_conflict",
                "github.store",
                "GitHub run profile differs from durable storage",
            ));
        }
        let version =
            i64::try_from(record.version).map_err(|_| storage_error("github_run_revision"))?;
        let next_version = version
            .checked_add(1)
            .ok_or_else(|| storage_error("github_run_revision"))?;
        let bytes = encode(record, self.limits.maximum_record_bytes)?;
        let updated = self.connection.execute("UPDATE github_runs SET version=?1, phase=?2, work=?3, record=?4 WHERE run_identity=?5 AND immutable_identity=?6 AND version=?7", params![next_version, record.phase.as_str(), record.has_work(), bytes, record.run_identity.as_str(), record.immutable_identity.as_str(), version]).map_err(|_| storage_error("github_run_update"))?;
        if updated != 1 {
            return Err(domain(
                "github_run_stale",
                "github.run",
                "GitHub run changed before its reserved transition",
            ));
        }
        record.version =
            u64::try_from(next_version).map_err(|_| storage_error("github_run_revision"))?;
        Ok(())
    }
}

// r[impl kiln.dashboard.repositories.github_export]
pub(crate) fn load_read_only(
    path: &Path,
    profile: &ContentId,
    run: &ContentId,
    maximum_record_bytes: usize,
) -> PortResult<RunRecord> {
    let maximum = sqlite_count(maximum_record_bytes)?;
    let mut connection =
        Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| storage_error("github_store_open"))?;
    connection
        .busy_timeout(Duration::from_millis(DATABASE_BUSY_TIMEOUT_MS))
        .map_err(|_| storage_error("github_store_timeout"))?;
    let transaction = connection
        .transaction()
        .map_err(|_| storage_error("github_store_transaction"))?;
    let version: i64 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| storage_error("github_store_version"))?;
    if version != DATABASE_VERSION {
        return Err(domain(
            "github_store_version",
            "github.store",
            "GitHub replay database version is unsupported",
        ));
    }
    let metadata = transaction
        .query_row(
            "SELECT schema, profile_identity FROM github_meta WHERE singleton=1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| storage_error("github_store_metadata"))?;
    if metadata.0 != DATABASE_SCHEMA || metadata.1 != profile.as_str() {
        return Err(domain(
            "github_store_profile_conflict",
            "github.store",
            "GitHub replay state belongs to a different schema or exact service profile",
        ));
    }
    // Bound the BLOB in SQLite before rusqlite allocates its bytes.
    let row = transaction
        .query_row(
            "SELECT version, CASE WHEN length(record)<=?2 THEN record ELSE NULL END, immutable_identity, phase, work FROM github_runs WHERE run_identity=?1",
            params![run.as_str(), maximum],
            |row| Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
            )),
        )
        .optional()
        .map_err(|_| storage_error("github_run_read"))?
        .ok_or_else(|| domain("github_run_missing", "github.run", "GitHub run does not exist"))?;
    let bytes = row.1.ok_or_else(|| {
        domain(
            "github_run_bound",
            "github.run",
            "GitHub stored run exceeds its durable byte bound",
        )
    })?;
    let record = decode(row.0, &bytes, profile, maximum_record_bytes)?;
    if &record.run_identity != run
        || record.immutable_identity.as_str() != row.2
        || record.phase.as_str() != row.3
        || record.has_work() != row.4
    {
        return Err(domain(
            "github_run_record",
            "github.run",
            "GitHub stored row disagrees with its validated run",
        ));
    }
    Ok(record)
}

fn stored_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, Vec<u8>)> {
    Ok((row.get(0)?, row.get(1)?))
}

fn encode(record: &RunRecord, maximum: usize) -> PortResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record).map_err(|_| storage_error("github_run_encoding"))?;
    if bytes.len() > maximum {
        return Err(domain(
            "github_run_bound",
            "github.run",
            "GitHub run exceeds its durable byte bound",
        ));
    }
    Ok(bytes)
}

fn decode(
    version: i64,
    bytes: &[u8],
    profile: &ContentId,
    maximum: usize,
) -> PortResult<RunRecord> {
    if bytes.len() > maximum {
        return Err(domain(
            "github_run_bound",
            "github.run",
            "GitHub stored run exceeds its durable byte bound",
        ));
    }
    let mut record: RunRecord =
        serde_json::from_slice(bytes).map_err(|_| storage_error("github_run_decoding"))?;
    record.version = u64::try_from(version).map_err(|_| storage_error("github_run_revision"))?;
    if &record.profile_identity != profile {
        return Err(domain(
            "github_store_profile_conflict",
            "github.store",
            "GitHub run profile differs from durable storage",
        ));
    }
    record.restore_state()?;
    Ok(record)
}

fn sqlite_count(value: usize) -> PortResult<i64> {
    i64::try_from(value).map_err(|_| {
        domain(
            "github_store_bound",
            "github.store",
            "GitHub storage bound exceeds SQLite count semantics",
        )
    })
}
fn storage_error(code: &str) -> kiln_provider_api::PortFailure {
    infrastructure(code, "GitHub durable storage operation failed")
}
fn retention_error() -> kiln_provider_api::PortFailure {
    domain(
        "github_replay_capacity",
        "github.store",
        "GitHub replay retention is full; no replay records were evicted",
    )
}
