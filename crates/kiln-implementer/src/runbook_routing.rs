//! Durable two-layer idempotency, reply-intent journaling, and bounded audit
//! ledger for GitHub issue/comment runbook routing (`OnixResearch/kiln#99`).
//!
//! Kept separate from [`crate::Implementer`] so `Implementer`'s reviewed
//! `ENTRY_POINTS` authority surface stays unchanged.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

const BUSY_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_LOCK_WAIT_MS: u64 = 120_000;
const LOCK_POLL_INTERVAL_MS: u64 = 25;
const MAX_STORE_ENTRIES: usize = 65_536;
const MAX_AUDIT_ENTRIES: usize = 10_000;
const MAX_TEXT_BYTES: usize = 256;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS runbook_deliveries (
    delivery_id TEXT PRIMARY KEY,
    body_identity TEXT NOT NULL,
    event TEXT NOT NULL,
    outcome TEXT NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS runbook_dispatches (
    source_key TEXT PRIMARY KEY,
    request_id TEXT NOT NULL UNIQUE,
    trigger_kind TEXT NOT NULL,
    repository_id INTEGER NOT NULL,
    issue_number INTEGER NOT NULL,
    runbook_id TEXT NOT NULL,
    dispatch_mode TEXT NOT NULL,
    dispatch_phase TEXT NOT NULL,
    dispatch_id TEXT,
    source_revision TEXT,
    reply_phase TEXT NOT NULL,
    reply_comment_id INTEGER,
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS runbook_routing_audit (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at_ms INTEGER NOT NULL,
    repository_id INTEGER NOT NULL,
    repository TEXT NOT NULL,
    actor TEXT NOT NULL,
    trigger_kind TEXT NOT NULL,
    issue_number INTEGER NOT NULL,
    comment_id INTEGER,
    label_name TEXT,
    runbook_id TEXT,
    outcome TEXT NOT NULL,
    decision_code TEXT NOT NULL,
    request_id TEXT,
    dispatch_id TEXT
);";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchPhase {
    Reserved,
    Dispatched,
    Failed,
}

impl DispatchPhase {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "dispatched" => Ok(Self::Dispatched),
            "failed" => Ok(Self::Failed),
            _ => Err("stored dispatch phase is invalid".to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyPhase {
    Pending,
    Reserved,
    Published,
    Skipped,
    Failed,
}

impl ReplyPhase {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "pending" => Ok(Self::Pending),
            "reserved" => Ok(Self::Reserved),
            "published" => Ok(Self::Published),
            "skipped" => Ok(Self::Skipped),
            "failed" => Ok(Self::Failed),
            _ => Err("stored reply phase is invalid".to_string()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryReservation {
    Fresh,
    Duplicate { outcome: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryStoreError {
    Conflict,
    Capacity,
    Invalid(String),
    Storage(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DispatchRecord {
    pub source_key: String,
    pub request_id: String,
    pub trigger_kind: String,
    pub repository_id: u64,
    pub issue_number: u64,
    pub runbook_id: String,
    pub dispatch_mode: String,
    pub dispatch_phase: DispatchPhase,
    pub dispatch_id: Option<String>,
    pub source_revision: Option<String>,
    pub reply_phase: ReplyPhase,
    pub reply_comment_id: Option<u64>,
    pub error_code: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewDispatchReservation {
    pub source_key: String,
    pub request_id: String,
    pub trigger_kind: String,
    pub repository_id: u64,
    pub issue_number: u64,
    pub runbook_id: String,
    pub dispatch_mode: String,
    pub now_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchReservation {
    Reserved(DispatchRecord),
    Existing(DispatchRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplyReservation {
    ReadyToPost,
    NeedsReconciliation,
    AlreadyPublished { comment_id: Option<u64> },
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NewAuditEntry {
    pub recorded_at_ms: u64,
    pub repository_id: u64,
    pub repository: String,
    pub actor: String,
    pub trigger_kind: String,
    pub issue_number: u64,
    pub comment_id: Option<u64>,
    pub label_name: Option<String>,
    pub runbook_id: Option<String>,
    pub outcome: String,
    pub decision_code: String,
    pub request_id: Option<String>,
    pub dispatch_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoutingAuditEntry {
    pub seq: u64,
    pub recorded_at_ms: u64,
    pub repository_id: u64,
    pub repository: String,
    pub actor: String,
    pub trigger_kind: String,
    pub issue_number: u64,
    pub comment_id: Option<u64>,
    pub label_name: Option<String>,
    pub runbook_id: Option<String>,
    pub outcome: String,
    pub decision_code: String,
    pub request_id: Option<String>,
    pub dispatch_id: Option<String>,
}

#[derive(Debug)]
pub struct RunbookRoutingLock {
    file: Option<File>,
}

impl Drop for RunbookRoutingLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.unlock();
        }
    }
}

#[derive(Debug)]
pub struct RunbookRoutingStore {
    connection: Connection,
    maximum_entries: usize,
    lock_path: Option<PathBuf>,
}

impl RunbookRoutingStore {
    pub fn open(path: &Path, maximum_entries: usize) -> Result<Self, String> {
        if maximum_entries == 0 || maximum_entries > MAX_STORE_ENTRIES {
            return Err("runbook routing store bound is zero or exceeds its maximum".to_string());
        }
        let connection = Connection::open(path).map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection.execute_batch(SCHEMA).map_err(text)?;
        Ok(Self {
            connection,
            maximum_entries,
            lock_path: derive_lock_path(path),
        })
    }

    pub fn acquire_lock(&self) -> Result<RunbookRoutingLock, String> {
        self.acquire_lock_with_timeout(Duration::from_millis(DEFAULT_LOCK_WAIT_MS))
    }

    pub fn acquire_lock_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<RunbookRoutingLock, String> {
        let Some(lock_path) = &self.lock_path else {
            return Ok(RunbookRoutingLock { file: None });
        };
        match std::fs::symlink_metadata(lock_path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("runbook routing lock path must not be a symlink".to_string());
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("runbook routing lock failed: {err}")),
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(lock_path)
            .map_err(|err| format!("runbook routing lock open failed: {err}"))?;
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(RunbookRoutingLock { file: Some(file) }),
                Err(TryLockError::WouldBlock) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err("runbook routing store is busy".to_string());
                    }
                    let remaining = deadline.saturating_duration_since(now);
                    std::thread::sleep(remaining.min(Duration::from_millis(LOCK_POLL_INTERVAL_MS)));
                }
                Err(TryLockError::Error(err)) => {
                    return Err(format!("runbook routing lock failed: {err}"));
                }
            }
        }
    }

    pub fn reserve_delivery(
        &mut self,
        delivery_id: &str,
        body_identity: &str,
        event: &str,
        now_ms: u64,
    ) -> Result<DeliveryReservation, DeliveryStoreError> {
        let normalized_delivery = delivery_id.to_ascii_lowercase();
        if !valid_delivery_id(&normalized_delivery)
            || !valid_hex64(body_identity)
            || event.is_empty()
            || event.len() > 64
        {
            return Err(DeliveryStoreError::Invalid(
                "delivery metadata is malformed or exceeds bounds".to_string(),
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| DeliveryStoreError::Storage(e.to_string()))?;
        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT body_identity, outcome FROM runbook_deliveries WHERE delivery_id = ?1",
                params![&normalized_delivery],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| DeliveryStoreError::Storage(e.to_string()))?;
        if let Some((stored_identity, outcome)) = existing {
            if stored_identity != body_identity {
                return Err(DeliveryStoreError::Conflict);
            }
            return Ok(DeliveryReservation::Duplicate { outcome });
        }
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runbook_deliveries", [], |row| {
                row.get(0)
            })
            .map_err(|e| DeliveryStoreError::Storage(e.to_string()))?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= self.maximum_entries {
            return Err(DeliveryStoreError::Capacity);
        }
        let now = i64::try_from(now_ms)
            .map_err(|_| DeliveryStoreError::Invalid("timestamp exceeds bound".to_string()))?;
        transaction
            .execute(
                "INSERT INTO runbook_deliveries
                 (delivery_id, body_identity, event, outcome, recorded_at_ms)
                 VALUES (?1, ?2, ?3, 'pending', ?4)",
                params![&normalized_delivery, body_identity, event, now],
            )
            .map_err(|e| DeliveryStoreError::Storage(e.to_string()))?;
        transaction
            .commit()
            .map_err(|e| DeliveryStoreError::Storage(e.to_string()))?;
        Ok(DeliveryReservation::Fresh)
    }

    pub fn set_delivery_outcome(
        &mut self,
        delivery_id: &str,
        outcome: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        let normalized_delivery = delivery_id.to_ascii_lowercase();
        if !matches!(outcome, "matched" | "ignored" | "refused" | "pending") {
            return Err("invalid delivery outcome".to_string());
        }
        self.connection
            .execute(
                "UPDATE runbook_deliveries SET outcome = ?2, recorded_at_ms = ?3 WHERE delivery_id = ?1",
                params![
                    normalized_delivery,
                    outcome,
                    i64::try_from(now_ms).map_err(text)?
                ],
            )
            .map_err(text)?;
        Ok(())
    }

    pub fn reserve_dispatch(
        &mut self,
        request: &NewDispatchReservation,
    ) -> Result<DispatchReservation, String> {
        if !valid_bounded_token(&request.source_key, MAX_TEXT_BYTES)
            || !valid_request_id(&request.request_id)
            || !matches!(request.trigger_kind.as_str(), "label" | "comment")
            || !valid_bounded_token(&request.runbook_id, 64)
            || !matches!(request.dispatch_mode.as_str(), "dispatch" | "fanout")
        {
            return Err("runbook dispatch reservation fields are invalid".to_string());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        if let Some(existing) = query_dispatch(&transaction, &request.source_key)? {
            return Ok(DispatchReservation::Existing(existing));
        }
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runbook_dispatches", [], |row| {
                row.get(0)
            })
            .map_err(text)?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= self.maximum_entries {
            return Err("runbook dispatch ledger is full".to_string());
        }
        let record = DispatchRecord {
            source_key: request.source_key.clone(),
            request_id: request.request_id.clone(),
            trigger_kind: request.trigger_kind.clone(),
            repository_id: request.repository_id,
            issue_number: request.issue_number,
            runbook_id: request.runbook_id.clone(),
            dispatch_mode: request.dispatch_mode.clone(),
            dispatch_phase: DispatchPhase::Reserved,
            dispatch_id: None,
            source_revision: None,
            reply_phase: ReplyPhase::Pending,
            reply_comment_id: None,
            error_code: None,
            updated_at_ms: request.now_ms,
        };
        transaction
            .execute(
                "INSERT INTO runbook_dispatches (
                    source_key, request_id, trigger_kind, repository_id, issue_number,
                    runbook_id, dispatch_mode, dispatch_phase, dispatch_id, source_revision,
                    reply_phase, reply_comment_id, error_code, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'reserved', NULL, NULL, 'pending', NULL, NULL, ?8)",
                params![
                    &record.source_key,
                    &record.request_id,
                    &record.trigger_kind,
                    i64::try_from(record.repository_id).map_err(text)?,
                    i64::try_from(record.issue_number).map_err(text)?,
                    &record.runbook_id,
                    &record.dispatch_mode,
                    i64::try_from(record.updated_at_ms).map_err(text)?,
                ],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)?;
        Ok(DispatchReservation::Reserved(record))
    }

    pub fn get_dispatch(&self, source_key: &str) -> Result<Option<DispatchRecord>, String> {
        query_dispatch(&self.connection, source_key)
    }

    pub fn mark_dispatched(
        &mut self,
        source_key: &str,
        dispatch_id: &str,
        source_revision: Option<&str>,
        now_ms: u64,
    ) -> Result<(), String> {
        if !valid_bounded_token(dispatch_id, 128)
            || source_revision.is_some_and(|rev| !valid_bounded_token(rev, 128))
        {
            return Err("dispatch outcome identifiers are invalid".to_string());
        }
        let updated = self
            .connection
            .execute(
                "UPDATE runbook_dispatches
                 SET dispatch_phase = 'dispatched',
                     dispatch_id = ?2,
                     source_revision = ?3,
                     error_code = NULL,
                     updated_at_ms = ?4
                 WHERE source_key = ?1",
                params![
                    source_key,
                    dispatch_id,
                    source_revision,
                    i64::try_from(now_ms).map_err(text)?,
                ],
            )
            .map_err(text)?;
        if updated != 1 {
            return Err("dispatch reservation not found".to_string());
        }
        Ok(())
    }

    pub fn mark_dispatch_failed(
        &mut self,
        source_key: &str,
        error_code: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if !valid_bounded_token(error_code, 128) {
            return Err("error code is invalid".to_string());
        }
        let updated = self
            .connection
            .execute(
                "UPDATE runbook_dispatches
                 SET dispatch_phase = 'failed',
                     reply_phase = 'skipped',
                     error_code = ?2,
                     updated_at_ms = ?3
                 WHERE source_key = ?1",
                params![source_key, error_code, i64::try_from(now_ms).map_err(text)?,],
            )
            .map_err(text)?;
        if updated != 1 {
            return Err("dispatch reservation not found".to_string());
        }
        Ok(())
    }

    pub fn reserve_reply(
        &mut self,
        source_key: &str,
        now_ms: u64,
    ) -> Result<ReplyReservation, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let Some(record) = query_dispatch(&transaction, source_key)? else {
            return Err("dispatch record not found for reply".to_string());
        };
        match record.reply_phase {
            ReplyPhase::Published => Ok(ReplyReservation::AlreadyPublished {
                comment_id: record.reply_comment_id,
            }),
            ReplyPhase::Reserved => Ok(ReplyReservation::NeedsReconciliation),
            ReplyPhase::Skipped => Ok(ReplyReservation::Skipped),
            ReplyPhase::Pending | ReplyPhase::Failed => {
                transaction
                    .execute(
                        "UPDATE runbook_dispatches
                         SET reply_phase = 'reserved',
                             updated_at_ms = ?2
                         WHERE source_key = ?1",
                        params![source_key, i64::try_from(now_ms).map_err(text)?],
                    )
                    .map_err(text)?;
                transaction.commit().map_err(text)?;
                Ok(ReplyReservation::ReadyToPost)
            }
        }
    }

    pub fn mark_reply_published(
        &mut self,
        source_key: &str,
        reply_comment_id: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        let updated = self
            .connection
            .execute(
                "UPDATE runbook_dispatches
                 SET reply_phase = 'published',
                     reply_comment_id = ?2,
                     error_code = NULL,
                     updated_at_ms = ?3
                 WHERE source_key = ?1",
                params![
                    source_key,
                    i64::try_from(reply_comment_id).map_err(text)?,
                    i64::try_from(now_ms).map_err(text)?,
                ],
            )
            .map_err(text)?;
        if updated != 1 {
            return Err("dispatch record not found".to_string());
        }
        Ok(())
    }

    pub fn mark_reply_failed(
        &mut self,
        source_key: &str,
        error_code: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        let bounded = truncate_token(error_code, 128);
        let updated = self
            .connection
            .execute(
                "UPDATE runbook_dispatches
                 SET reply_phase = 'failed',
                     error_code = ?2,
                     updated_at_ms = ?3
                 WHERE source_key = ?1",
                params![source_key, bounded, i64::try_from(now_ms).map_err(text)?,],
            )
            .map_err(text)?;
        if updated != 1 {
            return Err("dispatch record not found".to_string());
        }
        Ok(())
    }

    /// Record a diagnostic error during reply posting or reconciliation while
    /// preserving `reply_phase` (specifically keeping `'reserved'` monotonic
    /// until marker reconciliation resolves it).
    pub fn record_reply_reconciliation_error(
        &mut self,
        source_key: &str,
        error_code: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        let bounded = truncate_token(error_code, 128);
        let updated = self
            .connection
            .execute(
                "UPDATE runbook_dispatches
                 SET error_code = ?2,
                     updated_at_ms = ?3
                 WHERE source_key = ?1",
                params![source_key, bounded, i64::try_from(now_ms).map_err(text)?],
            )
            .map_err(text)?;
        if updated != 1 {
            return Err("dispatch record not found".to_string());
        }
        Ok(())
    }

    pub fn record_audit(&mut self, entry: &NewAuditEntry) -> Result<(), String> {
        let repository = truncate_token(&entry.repository, MAX_TEXT_BYTES);
        let actor = truncate_token(&entry.actor, 128);
        let trigger_kind = match entry.trigger_kind.as_str() {
            "label" | "comment" | "unknown" => entry.trigger_kind.as_str(),
            _ => "unknown",
        };
        let outcome = match entry.outcome.as_str() {
            "matched" | "ignored" | "refused" => entry.outcome.as_str(),
            _ => return Err("audit outcome is invalid".to_string()),
        };
        let decision_code = truncate_token(&entry.decision_code, 128);
        let label_name = entry
            .label_name
            .as_deref()
            .map(|value| truncate_token(value, MAX_TEXT_BYTES));
        let runbook_id = entry
            .runbook_id
            .as_deref()
            .map(|value| truncate_token(value, 64));
        let request_id = entry
            .request_id
            .as_deref()
            .map(|value| truncate_token(value, 128));
        let dispatch_id = entry
            .dispatch_id
            .as_deref()
            .map(|value| truncate_token(value, 128));

        let audit_cap = self.maximum_entries.min(MAX_AUDIT_ENTRIES);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runbook_routing_audit", [], |row| {
                row.get(0)
            })
            .map_err(text)?;
        let count_usize = usize::try_from(count).unwrap_or(usize::MAX);
        if count_usize >= audit_cap {
            let excess = i64::try_from(count_usize.saturating_sub(audit_cap) + 1).map_err(text)?;
            transaction
                .execute(
                    "DELETE FROM runbook_routing_audit
                     WHERE seq IN (SELECT seq FROM runbook_routing_audit ORDER BY seq ASC LIMIT ?1)",
                    params![excess],
                )
                .map_err(text)?;
        }
        transaction
            .execute(
                "INSERT INTO runbook_routing_audit (
                    recorded_at_ms, repository_id, repository, actor, trigger_kind,
                    issue_number, comment_id, label_name, runbook_id, outcome,
                    decision_code, request_id, dispatch_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    i64::try_from(entry.recorded_at_ms).map_err(text)?,
                    i64::try_from(entry.repository_id).map_err(text)?,
                    repository,
                    actor,
                    trigger_kind,
                    i64::try_from(entry.issue_number).map_err(text)?,
                    entry
                        .comment_id
                        .map(i64::try_from)
                        .transpose()
                        .map_err(text)?,
                    label_name,
                    runbook_id,
                    outcome,
                    decision_code,
                    request_id,
                    dispatch_id,
                ],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)?;
        Ok(())
    }

    pub fn list_audit(&self, limit: usize) -> Result<Vec<RoutingAuditEntry>, String> {
        let bounded = limit.clamp(1, MAX_AUDIT_ENTRIES);
        let mut statement = self
            .connection
            .prepare(
                "SELECT seq, recorded_at_ms, repository_id, repository, actor, trigger_kind,
                        issue_number, comment_id, label_name, runbook_id, outcome,
                        decision_code, request_id, dispatch_id
                 FROM runbook_routing_audit
                 ORDER BY seq ASC
                 LIMIT ?1",
            )
            .map_err(text)?;
        let rows = statement
            .query_map(params![i64::try_from(bounded).map_err(text)?], |row| {
                let comment_id: Option<i64> = row.get(7)?;
                Ok(RoutingAuditEntry {
                    seq: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                    recorded_at_ms: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    repository_id: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                    repository: row.get(3)?,
                    actor: row.get(4)?,
                    trigger_kind: row.get(5)?,
                    issue_number: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
                    comment_id: comment_id.and_then(|v| u64::try_from(v).ok()),
                    label_name: row.get(8)?,
                    runbook_id: row.get(9)?,
                    outcome: row.get(10)?,
                    decision_code: row.get(11)?,
                    request_id: row.get(12)?,
                    dispatch_id: row.get(13)?,
                })
            })
            .map_err(text)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(text)
    }
}

fn query_dispatch(conn: &Connection, source_key: &str) -> Result<Option<DispatchRecord>, String> {
    let row = conn
        .query_row(
            "SELECT source_key, request_id, trigger_kind, repository_id, issue_number,
                    runbook_id, dispatch_mode, dispatch_phase, dispatch_id, source_revision,
                    reply_phase, reply_comment_id, error_code, updated_at_ms
             FROM runbook_dispatches
             WHERE source_key = ?1",
            params![source_key],
            |row| {
                let phase_str: String = row.get(7)?;
                let reply_str: String = row.get(10)?;
                let reply_comment_id: Option<i64> = row.get(11)?;
                Ok((
                    DispatchRecord {
                        source_key: row.get(0)?,
                        request_id: row.get(1)?,
                        trigger_kind: row.get(2)?,
                        repository_id: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                        issue_number: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                        runbook_id: row.get(5)?,
                        dispatch_mode: row.get(6)?,
                        dispatch_phase: DispatchPhase::Reserved,
                        dispatch_id: row.get(8)?,
                        source_revision: row.get(9)?,
                        reply_phase: ReplyPhase::Pending,
                        reply_comment_id: reply_comment_id.and_then(|v| u64::try_from(v).ok()),
                        error_code: row.get(12)?,
                        updated_at_ms: u64::try_from(row.get::<_, i64>(13)?).unwrap_or(0),
                    },
                    phase_str,
                    reply_str,
                ))
            },
        )
        .optional()
        .map_err(text)?;
    match row {
        None => Ok(None),
        Some((mut record, phase_str, reply_str)) => {
            record.dispatch_phase = DispatchPhase::parse(&phase_str)?;
            record.reply_phase = ReplyPhase::parse(&reply_str)?;
            Ok(Some(record))
        }
    }
}

fn valid_delivery_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn valid_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_bounded_token(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn truncate_token(value: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_control() {
            continue;
        }
        if out.len() + ch.len_utf8() > max_bytes {
            break;
        }
        out.push(ch);
    }
    out
}

fn derive_lock_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .to_str()
            .is_some_and(|s| s == ":memory:" || s.starts_with("file:"))
    {
        return None;
    }
    let mut os = path.as_os_str().to_os_string();
    os.push(".routing-lock");
    Some(PathBuf::from(os))
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, RunbookRoutingStore) {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let store = RunbookRoutingStore::open(&dir.path().join("routing.sqlite"), 4)
            .unwrap_or_else(|_| unreachable!());
        (dir, store)
    }

    #[test]
    fn delivery_dedup_detects_duplicates_and_conflicts() {
        let (_dir, mut store) = store();
        let id = "00000000-0000-0000-0000-000000000001";
        let hash_a = "a".repeat(64);
        let hash_b = "b".repeat(64);
        assert_eq!(
            store.reserve_delivery(id, &hash_a, "issue_comment", 10),
            Ok(DeliveryReservation::Fresh)
        );
        store
            .set_delivery_outcome(id, "matched", 11)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            store.reserve_delivery(id, &hash_a, "issue_comment", 12),
            Ok(DeliveryReservation::Duplicate {
                outcome: "matched".to_string()
            })
        );
        assert_eq!(
            store.reserve_delivery(id, &hash_b, "issue_comment", 13),
            Err(DeliveryStoreError::Conflict)
        );
    }

    #[test]
    fn dispatch_and_reply_state_machine_prevents_double_post() {
        let (_dir, mut store) = store();
        let req = NewDispatchReservation {
            source_key: "comment:42:9001:flake-update".to_string(),
            request_id: format!("gh-rb-{}", "c".repeat(64)),
            trigger_kind: "comment".to_string(),
            repository_id: 42,
            issue_number: 7,
            runbook_id: "flake-update".to_string(),
            dispatch_mode: "dispatch".to_string(),
            now_ms: 100,
        };
        assert!(matches!(
            store.reserve_dispatch(&req),
            Ok(DispatchReservation::Reserved(_))
        ));
        assert!(matches!(
            store.reserve_dispatch(&req),
            Ok(DispatchReservation::Existing(_))
        ));
        store
            .mark_dispatched(&req.source_key, "web-run-1", Some("abc"), 101)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            store.reserve_reply(&req.source_key, 102),
            Ok(ReplyReservation::ReadyToPost)
        );
        // If crashed while reserved, next pass sees NeedsReconciliation.
        assert_eq!(
            store.reserve_reply(&req.source_key, 103),
            Ok(ReplyReservation::NeedsReconciliation)
        );
        // Recording a reconciliation error keeps reply_phase == Reserved.
        store
            .record_reply_reconciliation_error(&req.source_key, "github_comment_http_status", 103)
            .unwrap_or_else(|_| unreachable!());
        let after_err = store
            .get_dispatch(&req.source_key)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(after_err.reply_phase, ReplyPhase::Reserved);
        assert_eq!(
            after_err.error_code.as_deref(),
            Some("github_comment_http_status")
        );
        assert_eq!(
            store.reserve_reply(&req.source_key, 104),
            Ok(ReplyReservation::NeedsReconciliation)
        );
        store
            .mark_reply_published(&req.source_key, 555, 105)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            store.reserve_reply(&req.source_key, 106),
            Ok(ReplyReservation::AlreadyPublished {
                comment_id: Some(555)
            })
        );
    }

    #[test]
    fn audit_ring_buffer_bounds_entries_and_strips_control_chars() {
        let (_dir, mut store) = store();
        for seq in 0..6_u64 {
            store
                .record_audit(&NewAuditEntry {
                    recorded_at_ms: seq,
                    repository_id: 42,
                    repository: "onixresearch/kiln".to_string(),
                    actor: format!("actor-{seq}\n"),
                    trigger_kind: "comment".to_string(),
                    issue_number: seq + 1,
                    comment_id: Some(seq + 100),
                    label_name: None,
                    runbook_id: Some("flake-update".to_string()),
                    outcome: "refused".to_string(),
                    decision_code: "github_fork_denied".to_string(),
                    request_id: None,
                    dispatch_id: None,
                })
                .unwrap_or_else(|_| unreachable!());
        }
        let entries = store.list_audit(10).unwrap_or_else(|_| unreachable!());
        assert_eq!(entries.len(), 4);
        assert_eq!(entries.first().map(|e| e.issue_number), Some(3));
        assert_eq!(entries.first().map(|e| e.actor.as_str()), Some("actor-2"));
    }

    #[test]
    fn routing_lock_blocks_concurrent_holder_times_out_and_releases_on_drop() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let db_path = dir.path().join("routing.sqlite");
        let store1 = RunbookRoutingStore::open(&db_path, 8).unwrap_or_else(|_| unreachable!());
        let store2 = RunbookRoutingStore::open(&db_path, 8).unwrap_or_else(|_| unreachable!());
        let store3 = RunbookRoutingStore::open(&db_path, 8).unwrap_or_else(|_| unreachable!());

        let guard = store1
            .acquire_lock_with_timeout(Duration::from_millis(500))
            .unwrap_or_else(|_| unreachable!());
        // Read-only SQLite access on another handle must not deadlock while lock is held.
        assert!(store3.list_audit(10).is_ok());

        let err = store2
            .acquire_lock_with_timeout(Duration::from_millis(60))
            .err()
            .unwrap_or_else(|| unreachable!());
        assert!(err.contains("busy"), "expected busy error, got: {err}");

        let path_for_thread = db_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let store_thread =
                RunbookRoutingStore::open(&path_for_thread, 8).unwrap_or_else(|_| unreachable!());
            let _ = tx.send(());
            let started = Instant::now();
            let acquired = store_thread
                .acquire_lock_with_timeout(Duration::from_secs(2))
                .unwrap_or_else(|_| unreachable!());
            let elapsed = started.elapsed();
            drop(acquired);
            elapsed
        });
        rx.recv().unwrap_or_else(|_| unreachable!());
        std::thread::sleep(Duration::from_millis(120));
        drop(guard);

        let elapsed = waiter.join().unwrap_or_else(|_| unreachable!());
        assert!(
            elapsed >= Duration::from_millis(80),
            "expected waiter to block until drop, elapsed={elapsed:?}"
        );

        let reacquired = store2.acquire_lock_with_timeout(Duration::from_millis(200));
        assert!(reacquired.is_ok());
    }

    #[test]
    fn routing_lock_releases_ownership_even_with_cloned_fd() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let db_path = dir.path().join("routing.sqlite");
        let store1 = RunbookRoutingStore::open(&db_path, 8).unwrap_or_else(|_| unreachable!());
        let store2 = RunbookRoutingStore::open(&db_path, 8).unwrap_or_else(|_| unreachable!());

        let guard = store1
            .acquire_lock_with_timeout(Duration::from_millis(500))
            .unwrap_or_else(|_| unreachable!());
        let cloned_fd = guard
            .file
            .as_ref()
            .unwrap_or_else(|| unreachable!())
            .try_clone()
            .unwrap_or_else(|_| unreachable!());

        assert!(
            store2
                .acquire_lock_with_timeout(Duration::from_millis(50))
                .is_err()
        );
        drop(guard);

        // Even with cloned_fd still open, explicit unlock() in Drop released the flock.
        let reacquired = store2.acquire_lock_with_timeout(Duration::from_millis(200));
        assert!(
            reacquired.is_ok(),
            "expected lock release despite cloned fd, got {reacquired:?}"
        );
        drop(reacquired);
        drop(cloned_fd);
    }
}
