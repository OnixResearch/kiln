use std::path::Path;

use kiln_core::types::ExternalRef;
use rusqlite::{Connection, params};

use crate::error::{HostBridgeError, Result};

const METADATA_SCHEMA_KEY: &str = "schema";
const METADATA_SCHEMA_VALUE: &str = "kiln.aspen-host.operation-ledger.v1";
const METADATA_COHORT_KEY: &str = "cohort_identity";
pub(super) const METADATA_TICK_KEY: &str = "logical_tick";
const INITIAL_LOGICAL_TICK: &str = "0";
const INTEGRITY_OK: &str = "ok";

pub(super) fn configure_connection(connection: &Connection) -> Result<()> {
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .and_then(|()| connection.pragma_update(None, "synchronous", "FULL"))
        .and_then(|()| connection.pragma_update(None, "foreign_keys", "ON"))
        .and_then(|()| connection.pragma_update(None, "trusted_schema", "OFF"))
        .map_err(|error| ledger_io("configure operation ledger durability", error))
}

pub(super) fn initialize_schema(connection: &Connection, cohort_identity: &str) -> Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE metadata(\
                 key TEXT PRIMARY KEY NOT NULL,\
                 value TEXT NOT NULL\
             ) STRICT;\
             CREATE TABLE operations(\
                 operation_key TEXT PRIMARY KEY NOT NULL,\
                 record_json BLOB NOT NULL,\
                 record_hash TEXT NOT NULL\
             ) STRICT;",
        )
        .map_err(|error| ledger_io("initialize operation ledger schema", error))?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| ledger_io("begin operation ledger metadata", error))?;
    for (key, value) in [
        (METADATA_SCHEMA_KEY, METADATA_SCHEMA_VALUE),
        (METADATA_COHORT_KEY, cohort_identity),
        (METADATA_TICK_KEY, INITIAL_LOGICAL_TICK),
    ] {
        transaction
            .execute(
                "INSERT INTO metadata(key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|error| ledger_io("write operation ledger metadata", error))?;
    }
    transaction
        .commit()
        .map_err(|error| ledger_io("commit operation ledger metadata", error))
}

pub(super) fn verify_integrity(connection: &Connection) -> Result<()> {
    let result = connection
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map_err(|error| ledger_io("check operation ledger integrity", error))?;
    if result != INTEGRITY_OK {
        return Err(HostBridgeError::State(
            "operation ledger integrity check failed".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn verify_metadata(connection: &Connection, cohort_identity: &str) -> Result<()> {
    if metadata_value(connection, METADATA_SCHEMA_KEY)? != METADATA_SCHEMA_VALUE
        || metadata_value(connection, METADATA_COHORT_KEY)? != cohort_identity
    {
        return Err(HostBridgeError::State(
            "operation ledger schema or cohort identity changed".to_string(),
        ));
    }
    metadata_value(connection, METADATA_TICK_KEY)?
        .parse::<u64>()
        .map_err(|_| HostBridgeError::State("stored logical tick is invalid".to_string()))?;
    Ok(())
}

pub(super) fn metadata_value(connection: &Connection, key: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| ledger_io("read operation ledger metadata", error))
}

pub(super) fn operation_key(operation_id: &ExternalRef) -> Result<String> {
    operation_id
        .validate()
        .map_err(|error| HostBridgeError::State(error.message))?;
    serde_json::to_string(operation_id)
        .map_err(|_| HostBridgeError::State("encode operation identity".to_string()))
}

pub(super) fn require_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| HostBridgeError::Io(format!("inspect {label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HostBridgeError::State(format!(
            "{label} is not an admitted regular file"
        )));
    }
    Ok(())
}

pub(super) fn ledger_io(label: &str, error: rusqlite::Error) -> HostBridgeError {
    HostBridgeError::Io(format!("{label}: {error}"))
}
