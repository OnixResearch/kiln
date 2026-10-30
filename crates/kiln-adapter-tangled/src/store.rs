//! Durable pipeline and token replay store (shell, SQLite).
//!
//! A pipeline's replay key is unique, so a redelivered push maps to the stored
//! pipeline. Used service-auth `jti`s are kept until their tokens expire.

use std::path::Path;

use kiln_provider_api::PortResult;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{domain, infrastructure};
use crate::pipeline::{PipelineRecord, RunPhase, has_work, restore};

const DATABASE_SCHEMA: &str = "kiln.tangled-store.v1";
const BUSY_TIMEOUT_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreLimits {
    pub maximum_pipelines: usize,
    pub maximum_tokens: usize,
    pub maximum_record_bytes: usize,
}

#[derive(Debug)]
pub struct SqliteStore {
    connection: Connection,
    limits: StoreLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Insert {
    Inserted,
    /// A pipeline with the same replay key exists.
    Existing(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenClaim {
    Fresh,
    Replayed,
}

fn storage(code: &str) -> kiln_provider_api::PortFailure {
    infrastructure(code, "Tangled store operation failed")
}

fn count(value: usize) -> PortResult<i64> {
    i64::try_from(value).map_err(|_| {
        domain(
            "tangled_store_bound",
            "store",
            "Store bound exceeds SQLite range",
        )
    })
}

impl SqliteStore {
    // r[impl kiln.tangled.replay]
    pub fn open(path: &Path, limits: StoreLimits) -> PortResult<Self> {
        let connection = Connection::open(path).map_err(|_| storage("tangled_store_open"))?;
        connection
            .busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(|_| storage("tangled_store_open"))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=FULL;
                 CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS pipelines(
                     seq INTEGER PRIMARY KEY AUTOINCREMENT,
                     pipeline_id TEXT NOT NULL UNIQUE,
                     repo TEXT NOT NULL,
                     replay_key TEXT NOT NULL UNIQUE,
                     has_work INTEGER NOT NULL,
                     record BLOB NOT NULL);
                 CREATE TABLE IF NOT EXISTS tokens(key TEXT PRIMARY KEY, expires INTEGER NOT NULL);",
            )
            .map_err(|_| storage("tangled_store_schema"))?;
        let schema: Option<String> = connection
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| storage("tangled_store_schema"))?;
        match schema.as_deref() {
            None => {
                connection
                    .execute(
                        "INSERT INTO meta(key, value) VALUES('schema', ?1)",
                        params![DATABASE_SCHEMA],
                    )
                    .map_err(|_| storage("tangled_store_schema"))?;
            }
            Some(DATABASE_SCHEMA) => {}
            Some(_) => {
                return Err(domain(
                    "tangled_store_schema",
                    "store",
                    "Store schema is not supported",
                ));
            }
        }
        Ok(Self { connection, limits })
    }

    fn encode(&self, record: &PipelineRecord) -> PortResult<Vec<u8>> {
        let bytes = serde_json::to_vec(record).map_err(|_| storage("tangled_store_encode"))?;
        if bytes.len() > self.limits.maximum_record_bytes {
            return Err(domain(
                "tangled_store_record",
                "store",
                "Pipeline record exceeds its bound",
            ));
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> PortResult<PipelineRecord> {
        let mut record: PipelineRecord = serde_json::from_slice(bytes).map_err(|_| {
            domain(
                "tangled_store_decode",
                "store",
                "Stored pipeline is unreadable",
            )
        })?;
        restore(&mut record).map_err(|_| {
            domain(
                "tangled_store_decode",
                "store",
                "Stored pipeline history is invalid",
            )
        })?;
        Ok(record)
    }

    // r[impl kiln.tangled.replay]
    /// Record a token id until it expires. `Replayed` when it was already used.
    pub fn claim_token(
        &mut self,
        issuer: &str,
        jti: &str,
        expires_unix: i64,
        now_unix: i64,
    ) -> PortResult<TokenClaim> {
        let key = blake3::hash(format!("{issuer}\n{jti}").as_bytes())
            .to_hex()
            .to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| storage("tangled_store_token"))?;
        transaction
            .execute("DELETE FROM tokens WHERE expires < ?1", params![now_unix])
            .map_err(|_| storage("tangled_store_token"))?;
        let held: i64 = transaction
            .query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))
            .map_err(|_| storage("tangled_store_token"))?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO tokens(key, expires) VALUES(?1, ?2)",
                params![key, expires_unix],
            )
            .map_err(|_| storage("tangled_store_token"))?;
        if inserted == 1 && held >= count(self.limits.maximum_tokens)? {
            // Refusing is safe: the knot retries once older tokens expire.
            return Err(infrastructure(
                "tangled_store_tokens_full",
                "Token replay window is full",
            ));
        }
        transaction
            .commit()
            .map_err(|_| storage("tangled_store_token"))?;
        Ok(if inserted == 1 {
            TokenClaim::Fresh
        } else {
            TokenClaim::Replayed
        })
    }

    pub fn find_replay(&self, replay_key: &str) -> PortResult<Option<String>> {
        self.connection
            .query_row(
                "SELECT pipeline_id FROM pipelines WHERE replay_key = ?1",
                params![replay_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| storage("tangled_store_lookup"))
    }

    // r[impl kiln.tangled.replay]
    pub fn insert(&mut self, record: &PipelineRecord) -> PortResult<Insert> {
        let bytes = self.encode(record)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| storage("tangled_store_insert"))?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT pipeline_id FROM pipelines WHERE replay_key = ?1",
                params![record.replay_key.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| storage("tangled_store_insert"))?;
        if let Some(existing) = existing {
            return Ok(Insert::Existing(existing));
        }
        let held: i64 = transaction
            .query_row("SELECT COUNT(*) FROM pipelines", [], |row| row.get(0))
            .map_err(|_| storage("tangled_store_insert"))?;
        if held >= count(self.limits.maximum_pipelines)? {
            return Err(infrastructure(
                "tangled_store_full",
                "Pipeline store is full",
            ));
        }
        transaction
            .execute(
                "INSERT INTO pipelines(pipeline_id, repo, replay_key, has_work, record) VALUES(?1, ?2, ?3, ?4, ?5)",
                params![record.pipeline_id, record.repo_did, record.replay_key.as_str(), has_work(record), bytes],
            )
            .map_err(|_| storage("tangled_store_insert"))?;
        transaction
            .commit()
            .map_err(|_| storage("tangled_store_insert"))?;
        Ok(Insert::Inserted)
    }

    pub fn load(&self, pipeline_id: &str) -> PortResult<Option<PipelineRecord>> {
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT record FROM pipelines WHERE pipeline_id = ?1",
                params![pipeline_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| storage("tangled_store_load"))?;
        bytes.map(|bytes| Self::decode(&bytes)).transpose()
    }

    pub fn save(&mut self, record: &PipelineRecord) -> PortResult<()> {
        let bytes = self.encode(record)?;
        let updated = self
            .connection
            .execute(
                "UPDATE pipelines SET has_work = ?2, record = ?3 WHERE pipeline_id = ?1",
                params![record.pipeline_id, has_work(record), bytes],
            )
            .map_err(|_| storage("tangled_store_save"))?;
        if updated == 1 {
            Ok(())
        } else {
            Err(domain(
                "tangled_store_missing",
                "store",
                "Pipeline to save does not exist",
            ))
        }
    }

    /// The next pipeline with work after `cursor`, in insertion order.
    pub fn next_work(&self, cursor: i64) -> PortResult<Option<(i64, String)>> {
        self.connection
            .query_row(
                "SELECT seq, pipeline_id FROM pipelines WHERE has_work = 1 AND seq > ?1 ORDER BY seq LIMIT 1",
                params![cursor],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| storage("tangled_store_work"))
    }

    pub fn pipeline_count(&self) -> PortResult<usize> {
        let held: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM pipelines", [], |row| row.get(0))
            .map_err(|_| storage("tangled_store_count"))?;
        usize::try_from(held).map_err(|_| storage("tangled_store_count"))
    }

    pub fn run_count(&self) -> PortResult<usize> {
        let mut statement = self
            .connection
            .prepare("SELECT record FROM pipelines")
            .map_err(|_| storage("tangled_store_count"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|_| storage("tangled_store_count"))?;
        let mut runs = 0;
        for row in rows {
            let record = Self::decode(&row.map_err(|_| storage("tangled_store_count"))?)?;
            runs += record.workflows.len();
        }
        Ok(runs)
    }

    /// Runs dispatched or awaiting reconciliation, across pipelines with work.
    pub fn active_runs(&self) -> PortResult<usize> {
        let mut statement = self
            .connection
            .prepare("SELECT record FROM pipelines WHERE has_work = 1")
            .map_err(|_| storage("tangled_store_count"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|_| storage("tangled_store_count"))?;
        let mut active = 0;
        for row in rows {
            let record = Self::decode(&row.map_err(|_| storage("tangled_store_count"))?)?;
            active += record
                .workflows
                .iter()
                .filter(|run| {
                    matches!(
                        run.phase,
                        RunPhase::DispatchReserved | RunPhase::Active | RunPhase::Unknown
                    )
                })
                .count();
        }
        Ok(active)
    }
}
