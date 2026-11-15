//! Read-only canonical implementation-work lookup for operator projections.
use kiln_core::implementation::WorkLedger;
use kiln_core::inventory::WorkNeedsAttachment;
use kiln_core::types::ContentId;
use kiln_provider_api::inventory::{InventoryReadFailure, InventoryWorkPort};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::path::Path;

const MAX_OBSERVATION_RECORD_BYTES: i64 = 16 * 1024 * 1024;

fn record(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
) -> Result<Option<Vec<u8>>, InventoryReadFailure> {
    let row: Option<(i64, Option<Vec<u8>>)> = transaction.query_row(
        "SELECT length(bytes), CASE WHEN length(bytes) <= ?2 THEN bytes ELSE NULL END FROM implementation_state WHERE kind = ?1",
        params![key, MAX_OBSERVATION_RECORD_BYTES], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|_| InventoryReadFailure::Unavailable)?;
    match row {
        Some((size, _)) if !(0..=MAX_OBSERVATION_RECORD_BYTES).contains(&size) => {
            Err(InventoryReadFailure::MalformedObservation)
        }
        Some((_, bytes)) => bytes
            .map(Some)
            .ok_or(InventoryReadFailure::MalformedObservation),
        None => Ok(None),
    }
}

/// Opens an existing canonical store read-only. No schema creation, immediate
/// transaction, claim, inventory collection or resource reservation is possible.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct CurrentWorkObservation {
    pub identity: ContentId,
    pub revision: u64,
}

pub struct ImplementationObservation {
    connection: Connection,
}
impl std::fmt::Debug for ImplementationObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImplementationObservation")
            .finish_non_exhaustive()
    }
}
impl ImplementationObservation {
    pub fn open(path: &Path) -> Result<Self, InventoryReadFailure> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| InventoryReadFailure::Unavailable)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(Self { connection })
    }
    pub fn current_work(
        &self,
        identity: &ContentId,
    ) -> Result<CurrentWorkObservation, InventoryReadFailure> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        let bytes =
            record(&transaction, crate::WORK_KIND)?.ok_or(InventoryReadFailure::StaleWork)?;
        let ledger: WorkLedger = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let current = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|current| &current == identity))
            .ok_or(InventoryReadFailure::StaleWork)?;
        let result = CurrentWorkObservation {
            identity: identity.clone(),
            revision: current.revision,
        };
        transaction
            .commit()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(result)
    }
}
impl InventoryWorkPort for ImplementationObservation {
    fn current_requirements(
        &self,
        identity: &ContentId,
    ) -> Result<Option<WorkNeedsAttachment>, InventoryReadFailure> {
        // One deferred read transaction freezes work and requirement rows at
        // the same snapshot without escalating a GET into a writer.
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        let bytes =
            record(&transaction, crate::WORK_KIND)?.ok_or(InventoryReadFailure::StaleWork)?;
        let ledger: WorkLedger = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let current = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == identity))
            .ok_or(InventoryReadFailure::StaleWork)?;
        let Some(policy_bytes) = record(&transaction, "fair-admission-v1")? else {
            return Ok(None);
        };
        let admission: kiln_core::fair_admission::AdmissionLedger =
            serde_json::from_slice(&policy_bytes)
                .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let Some(policy) = admission.policy else {
            return Ok(None);
        };
        let policy = policy
            .identity()
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let Some(bytes) = record(
            &transaction,
            &format!("needs:{}:{}", identity.as_str(), policy.as_str()),
        )?
        else {
            let historical: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM implementation_state WHERE kind LIKE ?1)",
                    params![format!("needs:{}:%", identity.as_str())],
                    |row| row.get(0),
                )
                .map_err(|_| InventoryReadFailure::Unavailable)?;
            return if historical {
                Err(InventoryReadFailure::StaleWork)
            } else {
                Ok(None)
            };
        };
        let attachment: Option<WorkNeedsAttachment> = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        if let Some(attachment) = &attachment {
            attachment
                .validate_for(identity, current.revision, &policy)
                .map_err(|_| InventoryReadFailure::StaleWork)?;
        }
        transaction
            .commit()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(attachment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_only_open_never_creates_a_database() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("absent.sqlite");
        assert!(ImplementationObservation::open(&path).is_err());
        assert!(!path.exists());
    }
}
