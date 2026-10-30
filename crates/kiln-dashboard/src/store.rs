//! The SQLite event store input of the dashboard shell.
//!
//! The store is opened read-only. Its records carry the canonical JSON event
//! identity convention, and the store itself verifies those identities and
//! replays the retained history before the dashboard projects any view.

use std::path::Path;

use kiln_adapter_sqlite::{SqliteEventStore, SqliteProfile, replay_checked};
use kiln_core::domain::State;
use kiln_core::types::ExternalRef;

use crate::journal::AdmittedJournal;

/// The schema label reported for a store-backed journal.
pub const STORE_JOURNAL_SCHEMA: &str = "kiln.sqlite-event-store.v1";

/// Read one stream from a read-only event store and admit it.
// r[impl kiln.observe.dashboard.store]
// r[impl kiln.observe.dashboard.store.scenario.match]
// r[impl kiln.observe.dashboard.store.scenario.read_only]
pub fn admit_store(
    path: &Path,
    profile: &SqliteProfile,
    stream: &ExternalRef,
) -> Result<AdmittedJournal, String> {
    let store = SqliteEventStore::open_read_only(path, profile.clone())
        .map_err(|error| format!("event store open: {error:?}"))?;
    let records = store
        .load_stream_records(stream, profile.max_replay_records)
        .map_err(|error| format!("event store load: {error:?}"))?;
    let state = replay_checked(profile, &State::default(), &records)
        .map_err(|error| format!("event store replay: {}: {}", error.code, error.message))?;
    Ok(AdmittedJournal {
        state,
        records,
        schema: Some(STORE_JOURNAL_SCHEMA.to_string()),
        declared_identity: None,
        review: None,
    })
}

/// Parse the dashboard's own external reference spelling.
///
/// The spelling is `namespace:kind:id`, the same form the dashboard renders.
pub fn parse_reference(value: &str) -> Result<ExternalRef, String> {
    let mut parts = value.splitn(3, ':');
    let namespace = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let id = parts.next().unwrap_or_default();
    if namespace.is_empty() || kind.is_empty() || id.is_empty() {
        return Err(format!(
            "stream reference must be namespace:kind:id: {value}"
        ));
    }
    ExternalRef::new(namespace, kind, id)
        .map_err(|error| format!("stream reference: {}", error.message))
}
