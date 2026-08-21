use std::path::Path;

use kiln_core::types::{Diagnostic, RunId};
use kiln_core::workflow::{WorkflowEffectPlan, WorkflowObservation};
use kiln_provider_api::{PortFailure, PortResult, TriggerRequest, TriggerRun};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

const REPLAY_SCHEMA: &str = "kiln.radicle-replay.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayState {
    Reserved,
    Accepted,
    Uncertain,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRecord {
    pub schema: String,
    pub event_identity: String,
    pub trigger: TriggerRequest,
    pub plan: WorkflowEffectPlan,
    pub run: TriggerRun,
    pub state: ReplayState,
    pub observation: Option<WorkflowObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReserveResult {
    Inserted(ReplayRecord),
    Existing(ReplayRecord),
}

pub trait ReplayStore {
    fn reserve(&mut self, record: ReplayRecord) -> PortResult<ReserveResult>;
    fn load_by_run(&self, run_id: &RunId) -> PortResult<Option<ReplayRecord>>;
    fn replace(&mut self, expected: &ReplayRecord, next: ReplayRecord) -> PortResult<()>;
}

#[derive(Debug)]
pub struct SqliteReplayStore {
    connection: Connection,
    maximum_records: usize,
}

impl SqliteReplayStore {
    pub fn open(path: &Path, maximum_records: usize) -> PortResult<Self> {
        if !path.is_absolute() || maximum_records == 0 {
            return Err(domain(
                "radicle_replay_profile",
                "radicle.replay",
                "replay path must be absolute and its record bound must be positive",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| infrastructure("radicle_replay_parent"))?;
        }
        let connection =
            Connection::open(path).map_err(|_| infrastructure("radicle_replay_open"))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 CREATE TABLE IF NOT EXISTS radicle_replay (
                   event_identity TEXT PRIMARY KEY NOT NULL,
                   run_id TEXT UNIQUE NOT NULL,
                   record BLOB NOT NULL
                 );",
            )
            .map_err(|_| infrastructure("radicle_replay_schema"))?;
        Ok(Self {
            connection,
            maximum_records,
        })
    }
}

impl ReplayStore for SqliteReplayStore {
    // r[impl kiln.radicle.lifecycle]
    fn reserve(&mut self, record: ReplayRecord) -> PortResult<ReserveResult> {
        validate_record(&record)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| infrastructure("radicle_replay_transaction"))?;
        let existing = transaction
            .query_row(
                "SELECT record FROM radicle_replay WHERE event_identity = ?1",
                params![record.event_identity],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(|_| infrastructure("radicle_replay_read"))?;
        if let Some(bytes) = existing {
            let existing = decode(&bytes)?;
            if existing.trigger != record.trigger
                || existing.plan != record.plan
                || existing.run.run_id != record.run.run_id
            {
                return Err(domain(
                    "radicle_replay_conflict",
                    "radicle.trigger",
                    "event identity conflicts with durable replay material",
                ));
            }
            transaction
                .commit()
                .map_err(|_| infrastructure("radicle_replay_commit"))?;
            return Ok(ReserveResult::Existing(existing));
        }
        let count = transaction
            .query_row("SELECT COUNT(*) FROM radicle_replay", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| infrastructure("radicle_replay_count"))?;
        let maximum = i64::try_from(self.maximum_records).map_err(|_| {
            domain(
                "radicle_replay_bound",
                "radicle.replay.maximum_records",
                "replay record bound does not fit SQLite count semantics",
            )
        })?;
        if count >= maximum {
            return Err(PortFailure::Retryable(
                kiln_provider_api::InfrastructureFailure {
                    code: "radicle_replay_bound".to_string(),
                    message: "Radicle replay store reached its configured record bound".to_string(),
                },
            ));
        }
        let bytes = encode(&record)?;
        transaction
            .execute(
                "INSERT INTO radicle_replay (event_identity, run_id, record) VALUES (?1, ?2, ?3)",
                params![record.event_identity, record.run.run_id.as_str(), bytes],
            )
            .map_err(|_| infrastructure("radicle_replay_insert"))?;
        transaction
            .commit()
            .map_err(|_| infrastructure("radicle_replay_commit"))?;
        Ok(ReserveResult::Inserted(record))
    }

    fn load_by_run(&self, run_id: &RunId) -> PortResult<Option<ReplayRecord>> {
        let bytes = self
            .connection
            .query_row(
                "SELECT record FROM radicle_replay WHERE run_id = ?1",
                params![run_id.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(|_| infrastructure("radicle_replay_read"))?;
        bytes.map(|bytes| decode(&bytes)).transpose()
    }

    fn replace(&mut self, expected: &ReplayRecord, next: ReplayRecord) -> PortResult<()> {
        validate_record(&next)?;
        if expected.event_identity != next.event_identity
            || expected.trigger != next.trigger
            || expected.plan != next.plan
            || expected.run.run_id != next.run.run_id
        {
            return Err(domain(
                "radicle_replay_transition",
                "radicle.replay",
                "replay transition changed immutable trigger material",
            ));
        }
        let expected_bytes = encode(expected)?;
        let next_bytes = encode(&next)?;
        let updated = self
            .connection
            .execute(
                "UPDATE radicle_replay SET record = ?1 WHERE event_identity = ?2 AND record = ?3",
                params![next_bytes, expected.event_identity, expected_bytes],
            )
            .map_err(|_| infrastructure("radicle_replay_update"))?;
        if updated != 1 {
            return Err(domain(
                "radicle_replay_stale",
                "radicle.replay",
                "replay record changed before transition",
            ));
        }
        Ok(())
    }
}

fn validate_record(record: &ReplayRecord) -> PortResult<()> {
    if record.schema != REPLAY_SCHEMA || record.event_identity.is_empty() {
        return Err(domain(
            "radicle_replay_record",
            "radicle.replay",
            "replay record schema or event identity is invalid",
        ));
    }
    Ok(())
}

pub fn reserved_record(
    event_identity: String,
    trigger: TriggerRequest,
    plan: WorkflowEffectPlan,
    run: TriggerRun,
) -> ReplayRecord {
    ReplayRecord {
        schema: REPLAY_SCHEMA.to_string(),
        event_identity,
        trigger,
        plan,
        run,
        state: ReplayState::Reserved,
        observation: None,
    }
}

fn encode(record: &ReplayRecord) -> PortResult<Vec<u8>> {
    serde_json::to_vec(record).map_err(|_| infrastructure("radicle_replay_encode"))
}

fn decode(bytes: &[u8]) -> PortResult<ReplayRecord> {
    let record: ReplayRecord =
        serde_json::from_slice(bytes).map_err(|_| infrastructure("radicle_replay_decode"))?;
    validate_record(&record)?;
    Ok(record)
}

fn infrastructure(code: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: "Radicle replay storage is unavailable".to_string(),
    })
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::workflow::fixture_record;

    #[test]
    fn sqlite_replay_reserves_once_and_rejects_conflicting_transition() {
        let directory = tempdir().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("replay.sqlite");
        let mut store = SqliteReplayStore::open(&path, 2).unwrap_or_else(|_| unreachable!());
        let record = fixture_record('a');
        assert!(matches!(
            store.reserve(record.clone()),
            Ok(ReserveResult::Inserted(_))
        ));
        assert!(matches!(
            store.reserve(record.clone()),
            Ok(ReserveResult::Existing(_))
        ));
        let mut changed = record.clone();
        changed.event_identity = "different".to_string();
        assert!(store.replace(&record, changed).is_err());
    }
}
