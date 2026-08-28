use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use kiln_adapter_aspen::transport::{AspenServiceAck, AspenServiceIngress};
use kiln_core::types::ExternalRef;
use kiln_core::workflow::WorkflowObservation;
use rusqlite::{Connection, OpenFlags, params};

use super::model::{DurableEffectIntent, DurableOperationRecord};
use crate::error::{HostBridgeError, Result};

mod records;
mod sql;

use sql::{
    METADATA_TICK_KEY, configure_connection, initialize_schema, ledger_io, metadata_value,
    operation_key, require_regular_file, verify_integrity, verify_metadata,
};

const DATABASE_FILE_MODE: u32 = 0o600;
const BUSY_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerOpenMode {
    Fresh,
    Recover,
}

pub struct OperationLedger {
    connection: Connection,
    cohort_identity: String,
    maximum_records: usize,
    maximum_record_bytes: usize,
}

impl std::fmt::Debug for OperationLedger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OperationLedger")
            .field("cohort_identity", &self.cohort_identity)
            .field("maximum_records", &self.maximum_records)
            .field("maximum_record_bytes", &self.maximum_record_bytes)
            .finish_non_exhaustive()
    }
}

impl OperationLedger {
    pub fn open(
        path: &Path,
        mode: LedgerOpenMode,
        cohort_identity: &str,
        maximum_records: u64,
        maximum_record_bytes: u64,
    ) -> Result<Self> {
        let maximum_records = usize::try_from(maximum_records).map_err(|_| {
            HostBridgeError::Admission("operation record bound does not fit this host".to_string())
        })?;
        let maximum_record_bytes = usize::try_from(maximum_record_bytes).map_err(|_| {
            HostBridgeError::Admission("operation byte bound does not fit this host".to_string())
        })?;
        if cohort_identity.is_empty() || maximum_records == 0 || maximum_record_bytes == 0 {
            return Err(HostBridgeError::Admission(
                "operation ledger requires an identity and positive bounds".to_string(),
            ));
        }
        let flags = match mode {
            LedgerOpenMode::Fresh => {
                if std::fs::symlink_metadata(path).is_ok() {
                    return Err(HostBridgeError::State(
                        "fresh operation ledger path already exists".to_string(),
                    ));
                }
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX
            }
            LedgerOpenMode::Recover => {
                require_regular_file(path, "operation ledger")?;
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX
            }
        };
        let connection = Connection::open_with_flags(path, flags)
            .map_err(|error| ledger_io("open operation ledger", error))?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MILLISECONDS))
            .map_err(|error| ledger_io("configure operation ledger timeout", error))?;
        configure_connection(&connection)?;
        match mode {
            LedgerOpenMode::Fresh => {
                initialize_schema(&connection, cohort_identity)?;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(DATABASE_FILE_MODE))
                    .map_err(|error| {
                        HostBridgeError::Io(format!("set operation ledger permissions: {error}"))
                    })?;
            }
            LedgerOpenMode::Recover => {
                verify_integrity(&connection)?;
                verify_metadata(&connection, cohort_identity)?;
            }
        }
        let ledger = Self {
            connection,
            cohort_identity: cohort_identity.to_string(),
            maximum_records,
            maximum_record_bytes,
        };
        ledger.load_all()?;
        Ok(ledger)
    }

    pub fn reserve(&mut self, ingress: &AspenServiceIngress) -> Result<DurableOperationRecord> {
        if let Some(existing) = self.operation(&ingress.operation_id)? {
            if existing.ingress == *ingress {
                return Ok(existing);
            }
            return Err(HostBridgeError::State(
                "operation identity already names different ingress facts".to_string(),
            ));
        }
        if self.record_count()? >= self.maximum_records {
            return Err(HostBridgeError::State(
                "operation ledger reached its admitted record bound".to_string(),
            ));
        }
        let record =
            DurableOperationRecord::reserve(ingress.clone()).map_err(HostBridgeError::State)?;
        let (key, bytes, hash) = self.encoded_record(&record)?;
        self.connection
            .execute(
                "INSERT INTO operations(operation_key, record_json, record_hash) VALUES (?1, ?2, ?3)",
                params![key, bytes, hash],
            )
            .map_err(|error| ledger_io("reserve operation", error))?;
        Ok(record)
    }

    pub fn record_callback_receipt(
        &mut self,
        operation_id: &ExternalRef,
        callback_receipt_ref: &str,
    ) -> Result<DurableOperationRecord> {
        self.transition(operation_id, |record| {
            record.with_callback_receipt(callback_receipt_ref)
        })
    }

    pub fn record_dispatch_intent(
        &mut self,
        operation_id: &ExternalRef,
        intent: DurableEffectIntent,
    ) -> Result<DurableOperationRecord> {
        self.transition(operation_id, |record| record.with_dispatch_intent(intent))
    }

    pub fn record_provider_observation(
        &mut self,
        operation_id: &ExternalRef,
        observation: WorkflowObservation,
    ) -> Result<DurableOperationRecord> {
        self.transition(operation_id, |record| {
            record.with_provider_observation(observation)
        })
    }

    pub fn mark_uncertain(&mut self, operation_id: &ExternalRef) -> Result<DurableOperationRecord> {
        self.transition(operation_id, DurableOperationRecord::with_uncertainty)
    }

    pub fn record_acknowledgement(
        &mut self,
        operation_id: &ExternalRef,
        acknowledgement: AspenServiceAck,
    ) -> Result<DurableOperationRecord> {
        self.transition(operation_id, |record| {
            record.with_acknowledgement(acknowledgement)
        })
    }

    pub fn next_logical_tick(&mut self) -> Result<u64> {
        let current = self.logical_tick()?;
        let next = current
            .checked_add(1)
            .ok_or_else(|| HostBridgeError::State("host logical tick overflow".to_string()))?;
        let changed = self
            .connection
            .execute(
                "UPDATE metadata SET value = ?1 WHERE key = ?2 AND value = ?3",
                params![next.to_string(), METADATA_TICK_KEY, current.to_string()],
            )
            .map_err(|error| ledger_io("advance logical tick", error))?;
        if changed != 1 {
            return Err(HostBridgeError::State(
                "logical tick changed outside the admitted transaction".to_string(),
            ));
        }
        Ok(next)
    }

    pub fn logical_tick(&self) -> Result<u64> {
        let value = metadata_value(&self.connection, METADATA_TICK_KEY)?;
        value
            .parse::<u64>()
            .map_err(|_| HostBridgeError::State("stored logical tick is invalid".to_string()))
    }

    fn transition(
        &mut self,
        operation_id: &ExternalRef,
        transition: impl FnOnce(
            &DurableOperationRecord,
        ) -> std::result::Result<DurableOperationRecord, String>,
    ) -> Result<DurableOperationRecord> {
        let current = self.operation(operation_id)?.ok_or_else(|| {
            HostBridgeError::State("durable operation does not exist".to_string())
        })?;
        let next = transition(&current).map_err(HostBridgeError::State)?;
        if next == current {
            return Ok(current);
        }
        let key = operation_key(operation_id)?;
        let current_bytes = serde_json::to_vec(&current)
            .map_err(|_| HostBridgeError::State("encode current operation".to_string()))?;
        let current_hash = blake3::hash(&current_bytes).to_hex().to_string();
        let (_next_key, next_bytes, next_hash) = self.encoded_record(&next)?;
        let changed = self
            .connection
            .execute(
                "UPDATE operations SET record_json = ?1, record_hash = ?2 WHERE operation_key = ?3 AND record_hash = ?4",
                params![next_bytes, next_hash, key, current_hash],
            )
            .map_err(|error| ledger_io("commit operation transition", error))?;
        if changed != 1 {
            return Err(HostBridgeError::State(
                "operation changed outside the admitted transaction".to_string(),
            ));
        }
        Ok(next)
    }
}
