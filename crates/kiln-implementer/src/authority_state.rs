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
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_adapter_basalt::{REPLAY_DIGEST_BYTES, ReplayLedger, ReplayRecord, RevocationView};
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityDecision, AuthorityDenialReason, AuthorityProfile,
    AuthorityRequest, AuthoritySessionId, ProtectedAuthority,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use ucan::{HolderInvocationReplayError, ProofReference, RevocationError};

use super::text;

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
CREATE TABLE IF NOT EXISTS authority_revocation_view (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revocation_generation INTEGER NOT NULL,
    observed_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS authority_decision (
    seq INTEGER PRIMARY KEY,
    record TEXT NOT NULL
);";

/// Authority database path for one ledger file.
#[must_use]
pub fn authority_state_path(ledger: &Path) -> PathBuf {
    let mut name = ledger.as_os_str().to_owned();
    name.push(".");
    name.push(AUTHORITY_STATE_SUFFIX);
    PathBuf::from(name)
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

    /// Current revocation generation; zero before any revocation view.
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
/// Revocations, the revocation view and the decision log.
pub struct SqliteAuthorityState {
    connection: RefCell<Connection>,
}

impl std::fmt::Debug for SqliteAuthorityState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SqliteAuthorityState")
    }
}

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

    /// Record that the revocation view was observed current at `observed_at_ms`
    /// with `generation`. A view never moves to an older generation.
    ///
    /// # Errors
    /// Returns a message when the store cannot commit.
    pub fn observe_revocation_view(
        &self,
        generation: u64,
        observed_at_ms: u64,
    ) -> Result<(), String> {
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        transaction
            .execute(
                "INSERT INTO authority_revocation_view (id, revocation_generation, observed_at_ms)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET
                   revocation_generation = max(revocation_generation, excluded.revocation_generation),
                   observed_at_ms = excluded.observed_at_ms",
                params![sql_u64(generation)?, sql_u64(observed_at_ms)?],
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
}

impl AuthorityStateStore for SqliteAuthorityState {
    fn revoke(&self, proof_ref: [u8; REPLAY_DIGEST_BYTES], generation: u64) -> Result<(), String> {
        let now_ms = super::current_unix_ms();
        let mut connection = self.connection.borrow_mut();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        transaction
            .execute(
                "INSERT INTO authority_revocation (proof_ref, revoked_at_ms, revocation_generation)
                 VALUES (?1, ?2, ?3) ON CONFLICT(proof_ref) DO NOTHING",
                params![proof_ref.as_slice(), sql_u64(now_ms)?, sql_u64(generation)?],
            )
            .map_err(text)?;
        transaction
            .execute(
                "UPDATE authority_revocation_view
                 SET revocation_generation = max(revocation_generation, ?1) WHERE id = 1",
                params![sql_u64(generation)?],
            )
            .map_err(text)?;
        transaction.commit().map_err(text)
    }

    fn revocation_generation(&self) -> Result<u64, String> {
        let connection = self.connection.borrow();
        let generation: Option<i64> = connection
            .query_row(
                "SELECT revocation_generation FROM authority_revocation_view WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
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

/// Revocation view over `authority_revocation`, failing closed when the view
/// is absent or older than its maximum age.
#[derive(Debug)]
pub struct SqliteRevocationView {
    connection: RefCell<Connection>,
    max_age_ms: u64,
}

impl SqliteRevocationView {
    /// Open the view with its maximum age.
    ///
    /// # Errors
    /// Returns a message when the database cannot be opened.
    pub fn open(path: &Path, max_age_ms: u64) -> Result<Self, String> {
        Ok(Self {
            connection: RefCell::new(open(path)?),
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
                "SELECT observed_at_ms FROM authority_revocation_view WHERE id = 1",
                [],
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
