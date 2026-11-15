//! Selected-campaign owner integration: exact canonical claims and request seals.
use kiln_core::implementation::{
    AttemptLedger, DispatchRequestBinding, ImplementationAttempt, ImplementationWork, WorkLedger,
};
use kiln_core::types::ContentId;

use crate::{ATTEMPT_KIND, Implementer, WORK_KIND, text};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignClaimOutcome {
    Claimed(Box<ImplementationAttempt>),
    NotWinner { selected: Option<ContentId> },
}

impl Implementer {
    /// Claim only this selected work under its canonical parent-budget scope.
    pub fn claim_campaign_work(
        &mut self,
        work_identity: &ContentId,
        parent_budget_identity: &ContentId,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<CampaignClaimOutcome, String> {
        match self.claim_exact_winner(work_identity, parent_budget_identity, now_ms, lease_ms)? {
            crate::admission::AdmissionResult::Claimed {
                attempt: Some(attempt),
            } => Ok(CampaignClaimOutcome::Claimed(attempt)),
            crate::admission::AdmissionResult::NotWinner { selected } => {
                Ok(CampaignClaimOutcome::NotWinner { selected })
            }
            crate::admission::AdmissionResult::Claimed { attempt: None } => {
                Err("exact admission winner produced no durable attempt".into())
            }
            _ => Err("exact admission claim returned an unexpected result".into()),
        }
    }

    /// Seal exact request bytes to the retained plan before transport; transport
    /// results themselves are never accepted as campaign completion evidence.
    pub fn bind_campaign_request(
        &mut self,
        work: &ImplementationWork,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
        request_bytes: &[u8],
    ) -> Result<DispatchRequestBinding, String> {
        if request_bytes.is_empty() || request_bytes.len() > 2 * 1024 * 1024 {
            return Err("implementation request is empty or over its byte bound".into());
        }
        let request_digest =
            ContentId::from_blake3_hex(blake3::hash(request_bytes).to_hex().to_string())
                .map_err(crate::diagnostic)?;
        self.with_ledgers(|_, attempts| {
            attempts.bind_dispatch_request(work, attempt, fence, now_ms, &request_digest)
        })
    }

    pub fn campaign_request_binding(
        &mut self,
        attempt: &ContentId,
    ) -> Result<Option<DispatchRequestBinding>, String> {
        Ok(self
            .attempt(attempt)?
            .and_then(|attempt| attempt.dispatch_request_binding))
    }
}

/// Read one stable canonical ledger snapshot without opening a writer or
/// treating CLI/transport success JSON as authoritative evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCampaignLedgers {
    pub work: WorkLedger,
    pub attempts: AttemptLedger,
}
impl CanonicalCampaignLedgers {
    pub fn read(database: &std::path::Path) -> Result<Self, String> {
        use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
        let connection = Connection::open_with_flags(
            database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(text)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(text)?;
        let transaction = connection.unchecked_transaction().map_err(text)?;
        fn record<T: serde::de::DeserializeOwned>(
            transaction: &rusqlite::Transaction<'_>,
            kind: &str,
        ) -> Result<T, String> {
            let row: Option<(i64, Option<Vec<u8>>)> = transaction.query_row(
                "SELECT length(bytes),CASE WHEN length(bytes)<=16777216 THEN bytes ELSE NULL END FROM implementation_state WHERE kind=?1",
                params![kind], |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional().map_err(text)?;
            let Some((length, bytes)) = row else {
                return Err(format!("canonical {kind} ledger is unavailable"));
            };
            if !(0..=16_777_216).contains(&length) {
                return Err(format!("canonical {kind} ledger exceeds its read bound"));
            }
            serde_json::from_slice(
                &bytes.ok_or_else(|| format!("canonical {kind} ledger exceeds its read bound"))?,
            )
            .map_err(text)
        }
        let work = record::<WorkLedger>(&transaction, WORK_KIND)?;
        let attempts = record::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        transaction.commit().map_err(text)?;
        Ok(Self { work, attempts })
    }
}
