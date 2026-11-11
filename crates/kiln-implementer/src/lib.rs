//! Durable implementation work and attempt ledger.
//!
//! This shell owns the atomic claim, lease, fence and budget transitions that
//! `kiln-core` defines. It stores the work and attempt ledgers as versioned
//! JSON rows in SQLite and uses an immediate transaction for every mutation, so
//! two schedulers cannot both claim one work unit.
//!
//! Execution, workspace lifecycle and cancellation remain provider concerns.
//! This crate never runs a worker, verifies a candidate, publishes a pull
//! request, or grants release authority.

pub mod queue;

pub use queue::{DeliveryQueue, QueuedDelivery};

use std::path::Path;
use std::time::Duration;

use kiln_core::implementation::{
    AdmissionDecision, AttemptLedger, Enrollment, ImplementationAttempt, ImplementationWork,
    SourceMapping, WorkLedger,
};
use kiln_core::types::ContentId;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

const WORK_KIND: &str = "work";
const ATTEMPT_KIND: &str = "attempts";
const BUSY_TIMEOUT_MS: u64 = 10_000;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS implementation_state (
    kind TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    bytes BLOB NOT NULL
);";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    /// Attempts whose lease expired during this call.
    pub expired: Vec<ContentId>,
    /// Attempts still active and needing provider reconciliation.
    pub active: Vec<ContentId>,
}

pub struct Implementer {
    connection: Connection,
    enrollment: Enrollment,
}

impl std::fmt::Debug for Implementer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Implementer")
            .field("enrollment", &self.enrollment)
            .finish_non_exhaustive()
    }
}

impl Implementer {
    /// Open or create the durable ledger. The caller owns single-writer
    /// placement; conditional claims still serialize through SQLite.
    pub fn open(path: &Path, enrollment: Enrollment) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection.execute_batch(SCHEMA).map_err(text)?;
        Ok(Self {
            connection,
            enrollment,
        })
    }

    /// Admit one source observation through the pure work ledger.
    pub fn admit(
        &mut self,
        work: &ImplementationWork,
        mapping: Option<&SourceMapping>,
    ) -> Result<AdmissionDecision, String> {
        let enrollment = self.enrollment.clone();
        self.with_ledgers(|ledger, _| ledger.admit(work, mapping, &enrollment))
    }

    /// Claim the highest-ranked eligible work unit that is not already claimed.
    ///
    /// A second scheduler receives `None` when no other eligible work remains.
    // r[impl kiln.implementation.durable_store]
    pub fn claim_next(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<ImplementationAttempt>, String> {
        self.with_ledgers(|ledger, attempts| {
            let ranked = ledger.ranked();
            for row in ranked {
                let Some(work) = ledger
                    .current()
                    .find(|work| {
                        work.identity()
                            .is_ok_and(|identity| identity == row.identity)
                    })
                    .cloned()
                else {
                    continue;
                };
                match attempts.claim(&work, now_ms, lease_ms) {
                    Ok(attempt) => return Ok(Some(attempt)),
                    Err(error)
                        if matches!(
                            error.code.as_str(),
                            "implementation_already_claimed" | "implementation_budget_exhausted"
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(None)
        })
    }

    pub fn renew(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<u64, String> {
        self.with_ledgers(|_, attempts| attempts.renew(identity, fence, now_ms, lease_ms))
    }

    pub fn start_execution(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        workflow: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.start_execution(identity, fence, now_ms, workflow))
    }

    pub fn reconcile_dispatch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        observed: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| {
            attempts.reconcile_dispatch(identity, fence, now_ms, observed)
        })
    }

    pub fn produce_candidate(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        candidate: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| {
            attempts.produce_candidate(identity, fence, now_ms, candidate)
        })
    }

    pub fn begin_verification(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.begin_verification(identity, fence, now_ms))
    }

    pub fn fail(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        class: kiln_core::implementation::FailureClass,
        cause: &str,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.fail(identity, fence, now_ms, class, cause))
    }

    pub fn block(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.block(identity, fence, now_ms, cause))
    }

    pub fn cancel(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.cancel(identity, fence, now_ms, cause))
    }

    pub fn supersede(
        &mut self,
        canonical_key: &ContentId,
        new_revision: u64,
        rule: kiln_core::implementation::SupersessionRule,
    ) -> Result<Option<ContentId>, String> {
        self.with_ledgers(|_, attempts| attempts.supersede(canonical_key, new_revision, rule))
    }

    /// Expire ended leases and return the active/reconciliation set.
    // r[impl kiln.implementation.recovery]
    pub fn recover(&mut self, now_ms: u64) -> Result<Recovery, String> {
        self.with_ledgers(|_, attempts| {
            let expired = attempts.expire(now_ms)?;
            let active = attempts
                .attempts()
                .filter(|attempt| attempt.phase.is_active())
                .map(|attempt| attempt.identity.clone())
                .collect();
            Ok(Recovery { expired, active })
        })
    }

    pub fn work(&mut self) -> Result<WorkLedger, String> {
        self.with_ledgers(|ledger, _| Ok(ledger.clone()))
    }

    pub fn attempts(&mut self) -> Result<AttemptLedger, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.clone()))
    }

    pub fn attempt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<ImplementationAttempt>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.get(identity).cloned()))
    }

    fn with_ledgers<T>(
        &mut self,
        apply: impl FnOnce(
            &mut WorkLedger,
            &mut AttemptLedger,
        ) -> Result<T, kiln_core::types::Diagnostic>,
    ) -> Result<T, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut work, work_version) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let result = apply(&mut work, &mut attempts)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        save(&transaction, WORK_KIND, work_version, &work)?;
        save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
        transaction.commit().map_err(text)?;
        Ok(result)
    }
}

fn load<T: serde::de::DeserializeOwned + Default>(
    transaction: &Transaction<'_>,
    kind: &str,
) -> Result<(T, u64), String> {
    let row = transaction.query_row(
        "SELECT version, bytes FROM implementation_state WHERE kind = ?1",
        params![kind],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
    );
    match row {
        Ok((version, bytes)) => Ok((
            serde_json::from_slice(&bytes).map_err(text)?,
            u64::try_from(version).map_err(text)?,
        )),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok((T::default(), 0)),
        Err(error) => Err(text(error)),
    }
}

fn save<T: serde::Serialize>(
    transaction: &Transaction<'_>,
    kind: &str,
    version: u64,
    value: &T,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(text)?;
    let version = version
        .checked_add(1)
        .ok_or("implementation state version exhausted")?;
    let version = i64::try_from(version).map_err(text)?;
    transaction
        .execute(
            "INSERT INTO implementation_state (kind, version, bytes) VALUES (?1, ?2, ?3)
             ON CONFLICT(kind) DO UPDATE SET version = excluded.version, bytes = excluded.bytes",
            params![kind, version, bytes],
        )
        .map_err(text)?;
    Ok(())
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
