use kiln_adapter_aspen::protocol::CallbackEffectPlan;
use kiln_core::types::ExternalRef;
use rusqlite::{OptionalExtension, params};

use super::OperationLedger;
use super::sql::{ledger_io, operation_key};
use crate::durability::DurableOperationRecord;
use crate::error::{HostBridgeError, Result};

impl OperationLedger {
    pub fn operation(&self, operation_id: &ExternalRef) -> Result<Option<DurableOperationRecord>> {
        let key = operation_key(operation_id)?;
        let row = self
            .connection
            .query_row(
                "SELECT record_json, record_hash FROM operations WHERE operation_key = ?1",
                params![key],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| ledger_io("read operation", error))?;
        row.map(|(bytes, hash)| self.decode_record(&bytes, &hash))
            .transpose()
    }

    pub fn operation_for_effect(
        &self,
        plan: &CallbackEffectPlan,
    ) -> Result<Option<DurableOperationRecord>> {
        let mut matched = None;
        for record in self.load_all()? {
            if record.ingress.trigger_identity == plan.request_identity
                && record.ingress.idempotency_key == plan.idempotency_key
                && record.ingress.generation == plan.generation
            {
                if matched.is_some() {
                    return Err(HostBridgeError::State(
                        "effect plan matches multiple durable ingresses".to_string(),
                    ));
                }
                matched = Some(record);
            }
        }
        Ok(matched)
    }

    pub fn load_all(&self) -> Result<Vec<DurableOperationRecord>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT operation_key, record_json, record_hash FROM operations ORDER BY operation_key",
            )
            .map_err(|error| ledger_io("prepare operation scan", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| ledger_io("scan operation ledger", error))?;
        let mut records = Vec::new();
        for row in rows {
            let (key, bytes, hash) = row.map_err(|error| ledger_io("read operation row", error))?;
            if records.len() >= self.maximum_records {
                return Err(HostBridgeError::State(
                    "operation ledger exceeds its admitted record bound".to_string(),
                ));
            }
            let record = self.decode_record(&bytes, &hash)?;
            if operation_key(&record.ingress.operation_id)? != key {
                return Err(HostBridgeError::State(
                    "operation ledger key differs from its record".to_string(),
                ));
            }
            records.push(record);
        }
        Ok(records)
    }

    pub(super) fn encoded_record(
        &self,
        record: &DurableOperationRecord,
    ) -> Result<(String, Vec<u8>, String)> {
        record.validate().map_err(HostBridgeError::State)?;
        let bytes = serde_json::to_vec(record)
            .map_err(|_| HostBridgeError::State("encode durable operation".to_string()))?;
        if bytes.len() > self.maximum_record_bytes {
            return Err(HostBridgeError::State(
                "durable operation exceeds its admitted byte bound".to_string(),
            ));
        }
        let hash = blake3::hash(&bytes).to_hex().to_string();
        Ok((operation_key(&record.ingress.operation_id)?, bytes, hash))
    }

    pub(super) fn decode_record(&self, bytes: &[u8], hash: &str) -> Result<DurableOperationRecord> {
        if bytes.len() > self.maximum_record_bytes || blake3::hash(bytes).to_hex().as_str() != hash
        {
            return Err(HostBridgeError::State(
                "durable operation bytes exceed their bound or differ from identity".to_string(),
            ));
        }
        let record: DurableOperationRecord = serde_json::from_slice(bytes)
            .map_err(|_| HostBridgeError::State("durable operation JSON is invalid".to_string()))?;
        record.validate().map_err(HostBridgeError::State)?;
        Ok(record)
    }

    pub(super) fn record_count(&self) -> Result<usize> {
        let count = self
            .connection
            .query_row("SELECT COUNT(*) FROM operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| ledger_io("count operation records", error))?;
        usize::try_from(count).map_err(|_| {
            HostBridgeError::State("operation record count does not fit this host".to_string())
        })
    }
}
