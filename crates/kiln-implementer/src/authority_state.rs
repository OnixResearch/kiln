//! Durable implementation-authority state, `kiln.implementation-authority-state.v1`.
//!
//! Replay, revocation and redacted decisions live in the implementer's
//! authority database, a SQLite file beside the ledger
//! ([`authority_state_path`]). It is a separate file because protected
//! admission runs inside the ledger's `IMMEDIATE` transaction: a replay commit
//! on a second connection to the ledger file would wait on that same writer
//! lock. Each replay insert commits in its own `IMMEDIATE` transaction before
//! the effect, as Basalt's final consuming step requires.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_adapter_basalt::{REPLAY_DIGEST_BYTES, ReplayLedger, ReplayRecord, RevocationView};
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityDecision, AuthorityDenialReason, AuthorityProfile,
    AuthorityRequest, AuthoritySessionId, ProtectedAuthority,
};
use kiln_core::implementation::grant::{
    GrantLifecyclePhase, is_redacted_token, sanitize_redacted_denial_code,
};
use kiln_core::types::{ContentId, ExternalRef};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use ucan::{HolderInvocationReplayError, ProofReference, RevocationError};

use super::{SQLITE_HEADER_BYTES, sqlite_header_names_wal, text};
use crate::grant_issuance::{GRANT_ISSUANCE_SCHEMA, GrantIssuanceRecord, GrantIssuanceState};

/// Suffix of the authority database beside the ledger file.
pub const AUTHORITY_STATE_SUFFIX: &str = "authority";
/// Most persisted redacted decisions; older rows are pruned.
pub const MAX_PERSISTED_DECISIONS: u64 = 65_536;
/// Most decisions returned by one range read.
pub const MAX_DECISION_RANGE: u64 = 1_024;
/// Writer wait before a busy authority database denies.
const AUTHORITY_BUSY_TIMEOUT_MS: u64 = 10_000;

const AUTHORITY_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS authority_replay (
    audience TEXT NOT NULL,
    generation INTEGER NOT NULL,
    token_ref BLOB NOT NULL,
    nonce BLOB NOT NULL,
    resource_digest BLOB NOT NULL,
    expires_ms INTEGER NOT NULL,
    PRIMARY KEY (audience, generation, token_ref, nonce)
);
CREATE TABLE IF NOT EXISTS authority_revocation (
    proof_ref BLOB PRIMARY KEY,
    revoked_at_ms INTEGER NOT NULL,
    revocation_generation INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS authority_revocation_feed_view (
    feed TEXT PRIMARY KEY,
    revocation_generation INTEGER NOT NULL,
    observed_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS authority_decision (
    seq INTEGER PRIMARY KEY,
    record TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS grant_issuance_record (
    seq INTEGER PRIMARY KEY,
    schema TEXT NOT NULL,
    intent TEXT,
    grant_ref BLOB,
    record TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS grant_issuance_slot (
    intent TEXT PRIMARY KEY,
    parent_binding TEXT,
    account TEXT NOT NULL,
    parent_budget TEXT NOT NULL,
    work TEXT NOT NULL,
    attempt TEXT NOT NULL,
    fence INTEGER NOT NULL,
    depth INTEGER NOT NULL,
    grant_ref BLOB,
    binding_id TEXT,
    phase TEXT NOT NULL,
    revocation_generation INTEGER NOT NULL,
    unknown_termination TEXT
);
CREATE TABLE IF NOT EXISTS grant_issuance_denial (
    seq INTEGER PRIMARY KEY,
    work TEXT NOT NULL,
    attempt TEXT,
    fence INTEGER,
    reason TEXT NOT NULL,
    authority_generation INTEGER NOT NULL,
    revocation_generation INTEGER NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);";

/// Most persisted grant denial records returned by one query.
pub const MAX_GRANT_DENIALS_QUERY: usize = 64;

/// Materialized grant-issuance slot persisted in `grant_issuance_slot`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedGrantSlot {
    pub intent: ContentId,
    pub parent_binding: Option<ContentId>,
    pub account: ContentId,
    pub parent_budget: ContentId,
    pub work: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub depth: u8,
    pub grant_ref: Option<[u8; REPLAY_DIGEST_BYTES]>,
    pub binding_id: Option<ContentId>,
    pub phase: GrantLifecyclePhase,
    pub revocation_generation: u64,
    pub unknown_termination: Option<ExternalRef>,
}

/// Redacted grant-issuance or authority denial persisted in `grant_issuance_denial`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedGrantDenial {
    pub work: ContentId,
    pub attempt: Option<ContentId>,
    pub fence: Option<u64>,
    pub reason: String,
    pub authority_generation: u64,
    pub revocation_generation: u64,
    pub recorded_at_ms: u64,
}

impl RedactedGrantDenial {
    /// Construct a strictly redacted denial record.
    #[must_use]
    pub fn redacted(
        work: ContentId,
        attempt: Option<ContentId>,
        fence: Option<u64>,
        raw_reason: &str,
        authority_generation: u64,
        revocation_generation: u64,
        recorded_at_ms: u64,
    ) -> Self {
        Self {
            work,
            attempt,
            fence,
            reason: sanitize_redacted_denial_code(raw_reason),
            authority_generation,
            revocation_generation,
            recorded_at_ms,
        }
    }

    /// Validate that no unredacted path, token, or secret leaked into `reason`.
    ///
    /// # Errors
    /// Returns a message if any identity or redacted reason is malformed.
    pub fn validate(&self) -> Result<(), String> {
        self.work
            .validate()
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        if let Some(attempt) = &self.attempt {
            attempt
                .validate()
                .map_err(|error| format!("{}: {}", error.code, error.message))?;
        }
        if !is_redacted_token(&self.reason) {
            return Err("authority: denial reason must be a redacted token".into());
        }
        Ok(())
    }
}

/// Authority database path for one ledger file: beside the ledger's canonical
/// path, so every spelling of one ledger (a symlink, a relative path, `..`)
/// shares one replay, revocation and decision store.
///
/// # Errors
/// Returns a message when the ledger cannot be resolved, for example before
/// it exists.
pub fn authority_state_path(ledger: &Path) -> Result<PathBuf, String> {
    let mut name = std::fs::canonicalize(ledger)
        .map_err(|error| format!("authority: ledger path cannot be resolved: {error}"))?
        .into_os_string();
    name.push(".");
    name.push(AUTHORITY_STATE_SUFFIX);
    Ok(PathBuf::from(name))
}

/// The key of one configured revocation feed's view: the configured feed path
/// itself, validated and rebuilt from its components, never resolved through
/// the filesystem and never taken from anything the feed's content claims.
/// The feed is read from exactly this path, so no lookup separates the key
/// from the bytes read under it. Two spellings of one file get separate
/// views, each fresh only on its own imports, which fails closed; a symlinked
/// directory the producer re-points keeps its row.
///
/// The path must be absolute UTF-8 with no `.` or `..` component; empty
/// components (repeated `/`) normalize away.
///
/// # Errors
/// Returns a message for a relative, non-UTF-8 or dot-component path, or one
/// that names no component.
pub fn revocation_feed_key(feed: &Path) -> Result<String, String> {
    let configured = feed
        .to_str()
        .ok_or_else(|| "authority: revocation feed path is not UTF-8".to_string())?;
    if !configured.starts_with('/') {
        return Err("authority: revocation feed path must be absolute".into());
    }
    let mut key = String::with_capacity(configured.len());
    for component in configured.split('/').filter(|part| !part.is_empty()) {
        if component == "." || component == ".." {
            return Err("authority: revocation feed path must not contain `.` or `..`".into());
        }
        key.push('/');
        key.push_str(component);
    }
    if key.is_empty() {
        return Err("authority: revocation feed path names no file".into());
    }
    Ok(key)
}

fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(text)?;
    connection
        .busy_timeout(Duration::from_millis(AUTHORITY_BUSY_TIMEOUT_MS))
        .map_err(text)?;
    connection.execute_batch(AUTHORITY_SCHEMA).map_err(text)?;
    Ok(connection)
}

fn sql_u64(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(text)
}

/// Durable authority state shared by S5 and S6 consumers.
pub trait AuthorityStateStore {
    /// Record one revoked proof reference and raise the revocation generation.
    ///
    /// # Errors
    /// Returns a message when the store cannot commit.
    fn revoke(&self, proof_ref: [u8; REPLAY_DIGEST_BYTES], generation: u64) -> Result<(), String>;

    /// Highest revocation generation any feed view or revocation recorded;
    /// zero before either.
    ///
    /// # Errors
    /// Returns a message when the store cannot be read.
    fn revocation_generation(&self) -> Result<u64, String>;

    /// Persisted redacted decisions with sequence numbers in `range`, at most
    /// [`MAX_DECISION_RANGE`].
    ///
    /// # Errors
    /// Returns a message when the store cannot be read or a record is malformed.
    fn decisions(&self, range: Range<u64>) -> Result<Vec<(u64, AuthorityDecision)>, String>;
}

// r[impl kiln.implementation.authority.state]
/// Revocations, the per-feed revocation views and the decision log.
pub struct SqliteAuthorityState {
    connection: RefCell<Connection>,
}

impl std::fmt::Debug for SqliteAuthorityState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SqliteAuthorityState")
    }
}

/// Advance feed `?1`'s view row to `(?2 generation, ?3 observed_at_ms)` only
/// when that pair is newer, compared generation first: within one generation
/// a concurrent import of a stale copy never moves the feed's observation
/// back, and an equal pair is a no-op. A higher generation with an earlier
/// time does move the time back, which fails closed.
const ADVANCE_REVOCATION_VIEW: &str =
    "INSERT INTO authority_revocation_feed_view (feed, revocation_generation, observed_at_ms)
     VALUES (?1, ?2, ?3)
     ON CONFLICT(feed) DO UPDATE SET
       revocation_generation = excluded.revocation_generation,
       observed_at_ms = excluded.observed_at_ms
     WHERE (excluded.revocation_generation, excluded.observed_at_ms)
         > (authority_revocation_feed_view.revocation_generation,
            authority_revocation_feed_view.observed_at_ms)";

impl SqliteAuthorityState {
    /// Open or create the authority database.
    ///
    /// # Errors
    /// Returns a message when the database cannot be opened.
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            connection: RefCell::new(open(path)?),
        })
    }

    /// Open an existing authority database read-only without creating it or
    /// modifying a WAL-mode database. Returns `Ok(None)` when the file does
    /// not yet exist.
    ///
    /// # Errors
    /// Returns a message when the existing file is unreadable or in WAL mode.
    pub fn open_read_only(path: &Path) -> Result<Option<Self>, String> {
        use std::io::Read;
        if !path.exists() {
            return Ok(None);
        }
        let mut header = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(SQLITE_HEADER_BYTES).read_to_end(&mut header))
            .map_err(text)?;
        if sqlite_header_names_wal(&header) {
            return Err(
                "the authority database is in WAL mode, which a read-only open may change; it is refused"
                    .into(),
            );
        }
        let connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(AUTHORITY_BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(text)?;
        Ok(Some(Self {
            connection: RefCell::new(connection),
        }))
    }

    /// Record that feed `feed`'s view was observed current at
    /// `observed_at_ms` with `generation`. The view only moves to a newer
    /// `(generation, observed_at_ms)` pair.
    ///
    /// # Errors
    /// Returns a message when the store cannot commit.
    pub fn observe_revocation_view(
        &self,
        feed: &str,
        generation: u64,
        observed_at_ms: u64,
    ) -> Result<(), String> {
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        transaction
            .execute(
                ADVANCE_REVOCATION_VIEW,
                params![feed, sql_u64(generation)?, sql_u64(observed_at_ms)?],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)
    }

    /// Import one validated revocation feed atomically: every revoked
    /// reference and, when the feed's `(generation, observed_at_ms)` is newer
    /// than the view recorded for `feed`, that pair commit in one immediate
    /// transaction, or nothing does. Revocations are one union across every
    /// feed and are only ever added: an older or equal feed still adds the
    /// references it carries but leaves its view as it is, so a process
    /// importing a stale copy never moves the feed's observation back. Another
    /// feed's view is never touched.
    ///
    /// # Errors
    /// Returns a message for a failed commit; nothing is written then.
    pub fn import_revocation_feed(
        &self,
        feed: &str,
        generation: u64,
        observed_at_ms: u64,
        revoked: &[[u8; REPLAY_DIGEST_BYTES]],
    ) -> Result<(), String> {
        let now_ms = super::current_unix_ms();
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO authority_revocation (proof_ref, revoked_at_ms, revocation_generation)
                     VALUES (?1, ?2, ?3) ON CONFLICT(proof_ref) DO NOTHING",
                )
                .map_err(text)?;
            for reference in revoked {
                insert
                    .execute(params![
                        reference.as_slice(),
                        sql_u64(now_ms)?,
                        sql_u64(generation)?
                    ])
                    .map_err(text)?;
            }
        }
        transaction
            .execute(
                ADVANCE_REVOCATION_VIEW,
                params![feed, sql_u64(generation)?, sql_u64(observed_at_ms)?],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)
    }

    /// Append one redacted decision, pruning beyond [`MAX_PERSISTED_DECISIONS`].
    ///
    /// # Errors
    /// Returns a message when the record cannot be encoded or committed.
    pub fn record_decision(&self, decision: &AuthorityDecision) -> Result<u64, String> {
        let record = serde_json::to_string(decision).map_err(text)?;
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        transaction
            .execute(
                "INSERT INTO authority_decision (record) VALUES (?1)",
                params![record],
            )
            .map_err(text)?;
        let seq = transaction.last_insert_rowid();
        transaction
            .execute(
                "DELETE FROM authority_decision WHERE seq <= ?1",
                params![seq.saturating_sub(sql_u64(MAX_PERSISTED_DECISIONS)?)],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)?;
        u64::try_from(seq).map_err(text)
    }

    /// Load and replay all durable grant-issuance records together with all
    /// persisted revocations in `authority_revocation`.
    ///
    /// # Errors
    /// Returns a message when the database cannot be read or replayed.
    pub fn load_grant_issuance_state(&self) -> Result<GrantIssuanceState, String> {
        let connection = self.connection.borrow();
        load_grant_state_from_conn(&connection)
    }

    /// Atomically validate, journal, and materialize one grant-issuance record.
    ///
    /// # Errors
    /// Returns a message if the transition violates grant-issuance invariants or
    /// the database cannot commit; nothing is written on failure.
    pub fn record_grant_issuance(
        &self,
        record: &GrantIssuanceRecord,
    ) -> Result<GrantIssuanceState, String> {
        self.record_grant_issuance_batch(std::slice::from_ref(record))
    }

    /// Atomically validate, journal, and materialize a batch of grant-issuance
    /// records in one `IMMEDIATE` transaction.
    ///
    /// # Errors
    /// Returns a message if any record fails validation or state transition;
    /// the entire transaction rolls back.
    pub fn record_grant_issuance_batch(
        &self,
        records: &[GrantIssuanceRecord],
    ) -> Result<GrantIssuanceState, String> {
        let now_ms = super::current_unix_ms();
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let mut state = load_grant_state_from_conn(&transaction)?;
        for record in records {
            let mut next = state.clone();
            next.apply(record)
                .map_err(|error| format!("{}: {}", error.code, error.message))?;
            if next != state {
                let encoded = serde_json::to_string(record).map_err(text)?;
                let (intent_str, grant_ref_slice): (Option<String>, Option<&[u8]>) = match record {
                    GrantIssuanceRecord::Intended { grant } => {
                        (Some(grant.intent.as_str().to_string()), None)
                    }
                    GrantIssuanceRecord::Issued { intent, grant_ref } => (
                        Some(intent.as_str().to_string()),
                        Some(grant_ref.as_slice()),
                    ),
                    GrantIssuanceRecord::Revoked { grant_ref, .. } => {
                        (None, Some(grant_ref.as_slice()))
                    }
                    GrantIssuanceRecord::Abandoned { intent }
                    | GrantIssuanceRecord::Released { intent }
                    | GrantIssuanceRecord::UnknownTerminated { intent, .. }
                    | GrantIssuanceRecord::UnknownResolved { intent, .. } => {
                        (Some(intent.as_str().to_string()), None)
                    }
                };
                transaction
                    .execute(
                        "INSERT INTO grant_issuance_record (schema, intent, grant_ref, record)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![GRANT_ISSUANCE_SCHEMA, intent_str, grant_ref_slice, encoded],
                    )
                    .map_err(text)?;
            }
            sync_grant_slot_and_revocation(&transaction, &next, record, now_ms)?;
            state = next;
        }
        transaction.commit().map_err(text)?;
        Ok(state)
    }

    /// Read all persisted grant-issuance slots for `work`.
    ///
    /// # Errors
    /// Returns a message when the database cannot be read or decoded.
    pub fn grant_slots_for_work(
        &self,
        work: &ContentId,
    ) -> Result<Vec<PersistedGrantSlot>, String> {
        let connection = self.connection.borrow();
        if !table_exists(&connection, "grant_issuance_slot")? {
            return Ok(Vec::new());
        }
        let mut statement = connection
            .prepare(
                "SELECT intent, parent_binding, account, parent_budget, work, attempt,
                        fence, depth, grant_ref, binding_id, phase, revocation_generation,
                        unknown_termination
                 FROM grant_issuance_slot
                 WHERE work = ?1
                 ORDER BY fence ASC, depth ASC, intent ASC",
            )
            .map_err(text)?;
        let rows = statement
            .query_map(params![work.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<Vec<u8>>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })
            .map_err(text)?;
        let mut out = Vec::new();
        for row in rows {
            let (
                intent_s,
                parent_s,
                account_s,
                parent_budget_s,
                work_s,
                attempt_s,
                fence_i,
                depth_i,
                grant_ref_v,
                binding_s,
                phase_s,
                rev_gen_i,
                term_s,
            ) = row.map_err(text)?;
            let grant_ref = match grant_ref_v {
                Some(bytes) => {
                    let array: [u8; REPLAY_DIGEST_BYTES] = bytes.try_into().map_err(|_| {
                        "authority: malformed persisted grant_ref width".to_string()
                    })?;
                    Some(array)
                }
                None => None,
            };
            let phase: GrantLifecyclePhase =
                serde_json::from_str(&format!("\"{phase_s}\"")).map_err(text)?;
            let unknown_termination = term_s
                .map(|raw| serde_json::from_str::<ExternalRef>(&raw).map_err(text))
                .transpose()?;
            out.push(PersistedGrantSlot {
                intent: ContentId::from_blake3_hex(intent_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                parent_binding: parent_s
                    .map(ContentId::from_blake3_hex)
                    .transpose()
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                account: ContentId::from_blake3_hex(account_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                parent_budget: ContentId::from_blake3_hex(parent_budget_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                work: ContentId::from_blake3_hex(work_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                attempt: ContentId::from_blake3_hex(attempt_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                fence: u64::try_from(fence_i).map_err(text)?,
                depth: u8::try_from(depth_i).map_err(text)?,
                grant_ref,
                binding_id: binding_s
                    .map(ContentId::from_blake3_hex)
                    .transpose()
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                phase,
                revocation_generation: u64::try_from(rev_gen_i).map_err(text)?,
                unknown_termination,
            });
        }
        Ok(out)
    }

    /// Return all revoked proof references in `authority_revocation` and the
    /// current highest revocation generation.
    ///
    /// # Errors
    /// Returns a message when the store cannot be read.
    pub fn revoked_proof_refs(&self) -> Result<(BTreeSet<[u8; REPLAY_DIGEST_BYTES]>, u64), String> {
        let connection = self.connection.borrow();
        let mut refs = BTreeSet::new();
        let mut statement = connection
            .prepare("SELECT proof_ref FROM authority_revocation")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(text)?;
        for row in rows {
            let bytes = row.map_err(text)?;
            if let Ok(digest) = <[u8; REPLAY_DIGEST_BYTES]>::try_from(bytes.as_slice()) {
                refs.insert(digest);
            }
        }
        drop(statement);
        drop(connection);
        let generation = self.revocation_generation()?;
        Ok((refs, generation))
    }

    /// Append one redacted grant denial, pruning beyond [`MAX_PERSISTED_DECISIONS`].
    ///
    /// # Errors
    /// Returns a message if validation fails or the commit fails.
    pub fn record_grant_denial(&self, denial: &RedactedGrantDenial) -> Result<u64, String> {
        denial.validate()?;
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        transaction
            .execute(
                "INSERT INTO grant_issuance_denial
                 (work, attempt, fence, reason, authority_generation, revocation_generation, recorded_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    denial.work.as_str(),
                    denial.attempt.as_ref().map(ContentId::as_str),
                    denial.fence.map(sql_u64).transpose()?,
                    denial.reason,
                    sql_u64(denial.authority_generation)?,
                    sql_u64(denial.revocation_generation)?,
                    sql_u64(denial.recorded_at_ms)?,
                ],
            )
            .map_err(text)?;
        let seq = transaction.last_insert_rowid();
        transaction
            .execute(
                "DELETE FROM grant_issuance_denial WHERE seq <= ?1",
                params![seq.saturating_sub(sql_u64(MAX_PERSISTED_DECISIONS)?)],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)?;
        u64::try_from(seq).map_err(text)
    }

    /// Query bounded redacted grant denials for `work`, newest first.
    ///
    /// # Errors
    /// Returns a message when the database cannot be read or decoded.
    pub fn grant_denials_for_work(
        &self,
        work: &ContentId,
        limit: usize,
    ) -> Result<Vec<RedactedGrantDenial>, String> {
        let connection = self.connection.borrow();
        if !table_exists(&connection, "grant_issuance_denial")? {
            return Ok(Vec::new());
        }
        let capped = limit.min(MAX_GRANT_DENIALS_QUERY);
        let mut statement = connection
            .prepare(
                "SELECT work, attempt, fence, reason, authority_generation,
                        revocation_generation, recorded_at_ms
                 FROM grant_issuance_denial
                 WHERE work = ?1
                 ORDER BY seq DESC
                 LIMIT ?2",
            )
            .map_err(text)?;
        let rows = statement
            .query_map(
                params![work.as_str(), i64::try_from(capped).map_err(text)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .map_err(text)?;
        let mut out = Vec::new();
        for row in rows {
            let (work_s, attempt_s, fence_i, reason, auth_gen, rev_gen, rec_ms) =
                row.map_err(text)?;
            let denial = RedactedGrantDenial {
                work: ContentId::from_blake3_hex(work_s)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                attempt: attempt_s
                    .map(ContentId::from_blake3_hex)
                    .transpose()
                    .map_err(|e| format!("{}: {}", e.code, e.message))?,
                fence: fence_i.map(u64::try_from).transpose().map_err(text)?,
                reason,
                authority_generation: u64::try_from(auth_gen).map_err(text)?,
                revocation_generation: u64::try_from(rev_gen).map_err(text)?,
                recorded_at_ms: u64::try_from(rec_ms).map_err(text)?,
            };
            denial.validate()?;
            out.push(denial);
        }
        Ok(out)
    }
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(text)
}

fn load_grant_state_from_conn(connection: &Connection) -> Result<GrantIssuanceState, String> {
    let mut state = GrantIssuanceState::default();
    if table_exists(connection, "grant_issuance_record")? {
        let mut statement = connection
            .prepare("SELECT record FROM grant_issuance_record ORDER BY seq ASC")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(text)?;
        for row in rows {
            let raw = row.map_err(text)?;
            let record: GrantIssuanceRecord = serde_json::from_str(&raw).map_err(text)?;
            state
                .apply(&record)
                .map_err(|error| format!("{}: {}", error.code, error.message))?;
        }
    }
    if table_exists(connection, "authority_revocation")? {
        let mut statement = connection
            .prepare("SELECT proof_ref, revocation_generation FROM authority_revocation")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(text)?;
        for row in rows {
            let (bytes, gen_i) = row.map_err(text)?;
            if let Ok(grant_ref) = <[u8; REPLAY_DIGEST_BYTES]>::try_from(bytes.as_slice()) {
                let revocation_generation = u64::try_from(gen_i).map_err(text)?;
                state
                    .apply(&GrantIssuanceRecord::Revoked {
                        grant_ref,
                        revocation_generation,
                    })
                    .map_err(|error| format!("{}: {}", error.code, error.message))?;
            }
        }
    }
    Ok(state)
}

fn upsert_slot_for_intent(
    transaction: &Transaction<'_>,
    state: &GrantIssuanceState,
    intent: &ContentId,
    termination: Option<&ExternalRef>,
) -> Result<(), String> {
    let Some(grant) = state.grant(intent) else {
        return Ok(());
    };
    let grant_ref = state.issued(intent).copied();
    let binding_id = grant_ref
        .as_ref()
        .and_then(|gr| grant.binding_identity(gr).ok());
    let phase = state.lifecycle_phase(intent);
    let rev_gen = grant_ref
        .as_ref()
        .and_then(|gr| state.revocation_generation_of(gr))
        .unwrap_or(grant.revocation_generation);
    let term_json = termination
        .map(serde_json::to_string)
        .transpose()
        .map_err(text)?;
    transaction
        .execute(
            "INSERT INTO grant_issuance_slot
             (intent, parent_binding, account, parent_budget, work, attempt, fence, depth,
              grant_ref, binding_id, phase, revocation_generation, unknown_termination)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(intent) DO UPDATE SET
               grant_ref = COALESCE(excluded.grant_ref, grant_issuance_slot.grant_ref),
               binding_id = COALESCE(excluded.binding_id, grant_issuance_slot.binding_id),
               phase = excluded.phase,
               revocation_generation = MAX(grant_issuance_slot.revocation_generation, excluded.revocation_generation),
               unknown_termination = COALESCE(excluded.unknown_termination, grant_issuance_slot.unknown_termination)",
            params![
                intent.as_str(),
                grant.parent.as_ref().map(ContentId::as_str),
                grant.budget.account.as_str(),
                grant.budget.parent_budget_identity.as_str(),
                grant.work.as_str(),
                grant.attempt.as_str(),
                sql_u64(grant.fence)?,
                i64::from(grant.depth),
                grant_ref.as_ref().map(|gr| gr.as_slice()),
                binding_id.as_ref().map(ContentId::as_str),
                phase.as_str(),
                sql_u64(rev_gen)?,
                term_json,
            ],
        )
        .map(drop)
        .map_err(text)
}

fn sync_grant_slot_and_revocation(
    transaction: &Transaction<'_>,
    state: &GrantIssuanceState,
    record: &GrantIssuanceRecord,
    now_ms: u64,
) -> Result<(), String> {
    match record {
        GrantIssuanceRecord::Intended { grant } => {
            upsert_slot_for_intent(transaction, state, &grant.intent, None)
        }
        GrantIssuanceRecord::Issued { intent, .. }
        | GrantIssuanceRecord::Abandoned { intent }
        | GrantIssuanceRecord::Released { intent } => {
            upsert_slot_for_intent(transaction, state, intent, None)
        }
        GrantIssuanceRecord::UnknownTerminated {
            intent,
            revocation_generation,
        } => {
            if let Some(grant_ref) = state.issued(intent) {
                insert_revocation(transaction, grant_ref, now_ms, *revocation_generation)?;
            }
            upsert_slot_for_intent(transaction, state, intent, None)
        }
        GrantIssuanceRecord::UnknownResolved {
            intent,
            termination,
        } => upsert_slot_for_intent(transaction, state, intent, Some(termination)),
        GrantIssuanceRecord::Revoked {
            grant_ref,
            revocation_generation,
        } => {
            insert_revocation(transaction, grant_ref, now_ms, *revocation_generation)?;
            let mut matching_intents = Vec::new();
            {
                let mut statement = transaction
                    .prepare(
                        "SELECT intent FROM grant_issuance_slot
                         WHERE grant_ref = ?1 OR phase IN ('intended', 'issued')",
                    )
                    .map_err(text)?;
                let rows = statement
                    .query_map(params![grant_ref.as_slice()], |row| row.get::<_, String>(0))
                    .map_err(text)?;
                for row in rows {
                    matching_intents.push(
                        ContentId::from_blake3_hex(row.map_err(text)?)
                            .map_err(|e| format!("{}: {}", e.code, e.message))?,
                    );
                }
            }
            for intent in matching_intents {
                if state.is_intent_fenced_or_revoked(&intent) {
                    upsert_slot_for_intent(transaction, state, &intent, None)?;
                }
            }
            Ok(())
        }
    }
}

fn insert_revocation(
    transaction: &Transaction<'_>,
    proof_ref: &[u8; REPLAY_DIGEST_BYTES],
    now_ms: u64,
    revocation_generation: u64,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO authority_revocation (proof_ref, revoked_at_ms, revocation_generation)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(proof_ref) DO UPDATE SET
               revocation_generation = MAX(authority_revocation.revocation_generation, excluded.revocation_generation)",
            params![proof_ref.as_slice(), sql_u64(now_ms)?, sql_u64(revocation_generation)?],
        )
        .map(drop)
        .map_err(text)
}

impl AuthorityStateStore for SqliteAuthorityState {
    fn revoke(&self, proof_ref: [u8; REPLAY_DIGEST_BYTES], generation: u64) -> Result<(), String> {
        let now_ms = super::current_unix_ms();
        self.connection
            .borrow()
            .execute(
                "INSERT INTO authority_revocation (proof_ref, revoked_at_ms, revocation_generation)
                 VALUES (?1, ?2, ?3) ON CONFLICT(proof_ref) DO NOTHING",
                params![proof_ref.as_slice(), sql_u64(now_ms)?, sql_u64(generation)?],
            )
            .map(drop)
            .map_err(text)
    }

    fn revocation_generation(&self) -> Result<u64, String> {
        let connection = self.connection.borrow();
        let generation: Option<i64> = connection
            .query_row(
                "SELECT max(generation) FROM (
                   SELECT revocation_generation AS generation FROM authority_revocation_feed_view
                   UNION ALL
                   SELECT revocation_generation FROM authority_revocation)",
                [],
                |row| row.get(0),
            )
            .map_err(text)?;
        u64::try_from(generation.unwrap_or(0)).map_err(text)
    }

    fn decisions(&self, range: Range<u64>) -> Result<Vec<(u64, AuthorityDecision)>, String> {
        let end = range
            .end
            .min(range.start.saturating_add(MAX_DECISION_RANGE));
        let connection = self.connection.borrow();
        let mut statement = connection
            .prepare("SELECT seq, record FROM authority_decision WHERE seq >= ?1 AND seq < ?2 ORDER BY seq")
            .map_err(text)?;
        let rows = statement
            .query_map(params![sql_u64(range.start)?, sql_u64(end)?], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(text)?;
        let mut decisions = Vec::new();
        for row in rows {
            let (seq, record) = row.map_err(text)?;
            decisions.push((
                u64::try_from(seq).map_err(text)?,
                serde_json::from_str(&record).map_err(text)?,
            ));
        }
        Ok(decisions)
    }
}

// r[impl kiln.implementation.authority.state.scenario.replay]
/// Insert-or-deny replay ledger over `authority_replay`.
#[derive(Debug)]
pub struct SqliteReplayLedger {
    connection: RefCell<Connection>,
}

impl SqliteReplayLedger {
    /// Open or create the authority database.
    ///
    /// # Errors
    /// Returns a message when the database cannot be opened.
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            connection: RefCell::new(open(path)?),
        })
    }

    /// Open an existing authority database read-only without creating it or
    /// modifying a WAL-mode database. Returns `Ok(None)` when the file does
    /// not yet exist.
    ///
    /// # Errors
    /// Returns a message when the existing file is unreadable or in WAL mode.
    pub fn open_read_only(path: &Path) -> Result<Option<Self>, String> {
        use std::io::Read;
        if !path.exists() {
            return Ok(None);
        }
        let mut header = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(SQLITE_HEADER_BYTES).read_to_end(&mut header))
            .map_err(text)?;
        if sqlite_header_names_wal(&header) {
            return Err(
                "the authority database is in WAL mode, which a read-only open may change; it is refused"
                    .into(),
            );
        }
        let connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(AUTHORITY_BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(text)?;
        Ok(Some(Self {
            connection: RefCell::new(connection),
        }))
    }

    /// Recorded invocations, for audit.
    ///
    /// # Errors
    /// Returns a message when the store cannot be read.
    pub fn recorded(&self) -> Result<u64, String> {
        let connection = self.connection.borrow();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM authority_replay", [], |row| {
                row.get(0)
            })
            .map_err(text)?;
        u64::try_from(count).map_err(text)
    }
}

impl ReplayLedger for SqliteReplayLedger {
    fn consume(
        &self,
        record: &ReplayRecord<'_>,
        now_ms: u64,
    ) -> Result<(), HolderInvocationReplayError> {
        let backend = |_| HolderInvocationReplayError::Backend;
        let generation = i64::try_from(record.generation).map_err(backend)?;
        let expires_ms = i64::try_from(record.expires_ms).map_err(backend)?;
        let now = i64::try_from(now_ms).map_err(backend)?;
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| HolderInvocationReplayError::Backend)?;
        transaction
            .execute(
                "DELETE FROM authority_replay WHERE expires_ms < ?1",
                params![now],
            )
            .map_err(|_| HolderInvocationReplayError::Backend)?;
        let inserted = transaction
            .execute(
                "INSERT INTO authority_replay
                 (audience, generation, token_ref, nonce, resource_digest, expires_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(audience, generation, token_ref, nonce) DO NOTHING",
                params![
                    record.audience,
                    generation,
                    record.token_ref.as_slice(),
                    record.nonce.as_slice(),
                    record.resource_digest.as_slice(),
                    expires_ms
                ],
            )
            .map_err(|_| HolderInvocationReplayError::Backend)?;
        if inserted == 0 {
            return Err(HolderInvocationReplayError::Duplicate);
        }
        transaction
            .commit()
            .map_err(|_| HolderInvocationReplayError::Backend)
    }
}

// r[impl kiln.implementation.authority.state.scenario.stale]
/// Revocation view over the shared `authority_revocation` union, fresh only
/// by its own feed's observation: it fails closed when that feed's view is
/// absent or older than its maximum age, however recently another feed
/// sharing the store was imported.
#[derive(Debug)]
pub struct SqliteRevocationView {
    connection: RefCell<Connection>,
    feed: String,
    max_age_ms: u64,
}

impl SqliteRevocationView {
    /// Open the view of feed `feed` ([`revocation_feed_key`]) with its
    /// maximum age.
    ///
    /// # Errors
    /// Returns a message when the database cannot be opened.
    pub fn open(path: &Path, feed: impl Into<String>, max_age_ms: u64) -> Result<Self, String> {
        Ok(Self {
            connection: RefCell::new(open(path)?),
            feed: feed.into(),
            max_age_ms,
        })
    }
}

impl RevocationView for SqliteRevocationView {
    fn is_revoked(&self, reference: &ProofReference, now_ms: u64) -> Result<bool, RevocationError> {
        let failure = |message: &str| RevocationError::Backend {
            reference: reference.clone(),
            message: message.into(),
        };
        let connection = self.connection.borrow();
        let observed: Option<i64> = connection
            .query_row(
                "SELECT observed_at_ms FROM authority_revocation_feed_view WHERE feed = ?1",
                params![self.feed],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| failure("revocation view unreadable"))?;
        let fresh = observed
            .and_then(|observed| u64::try_from(observed).ok())
            .is_some_and(|observed| observed <= now_ms && now_ms - observed <= self.max_age_ms);
        if !fresh {
            return Err(RevocationError::Stale {
                reference: reference.clone(),
                message: "revocation view is absent or older than its maximum age".into(),
            });
        }
        connection
            .query_row(
                "SELECT 1 FROM authority_revocation WHERE proof_ref = ?1",
                params![reference.as_bytes()],
                |_| Ok(()),
            )
            .optional()
            .map(|row| row.is_some())
            .map_err(|_| failure("revocation table unreadable"))
    }
}

/// Protected-authority port that persists every decision it returns.
///
/// An allowed decision that cannot be persisted fails closed: the session is
/// closed and the request denies.
#[allow(missing_debug_implementations)]
pub struct RecordingAuthority {
    inner: Box<dyn ProtectedAuthority>,
    state: SqliteAuthorityState,
}

impl RecordingAuthority {
    /// Persist the decisions of `inner` into `state`.
    #[must_use]
    pub fn new(inner: Box<dyn ProtectedAuthority>, state: SqliteAuthorityState) -> Self {
        Self { inner, state }
    }

    fn persisted(&mut self, decision: &AuthorityDecision) -> bool {
        if self.state.record_decision(decision).is_ok() {
            return true;
        }
        if let Some(session) = decision.session {
            self.inner.close(session);
        }
        false
    }
}

impl ProtectedAuthority for RecordingAuthority {
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<AuthorityDecision, AuthorityDecision> {
        match self.inner.admit(request, profile, currentness, now_unix_s) {
            Ok(decision) => {
                if self.persisted(&decision) {
                    Ok(decision)
                } else {
                    Err(AuthorityDecision::deny(
                        request,
                        AuthorityDenialReason::VerifierUnavailable,
                    ))
                }
            }
            Err(decision) => {
                let _ = self.state.record_decision(&decision);
                Err(decision)
            }
        }
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<(), AuthorityDecision> {
        match self
            .inner
            .revalidate(session, request, currentness, now_unix_s)
        {
            Ok(()) => {
                let decision = self.inner.decisions().last().cloned();
                match decision {
                    Some(decision) if self.persisted(&decision) => Ok(()),
                    _ => {
                        self.inner.close(session);
                        Err(AuthorityDecision::deny(
                            request,
                            AuthorityDenialReason::VerifierUnavailable,
                        ))
                    }
                }
            }
            Err(decision) => {
                let _ = self.state.record_decision(&decision);
                Err(decision)
            }
        }
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.inner.close(session);
    }

    fn decisions(&self) -> &[AuthorityDecision] {
        self.inner.decisions()
    }
}
