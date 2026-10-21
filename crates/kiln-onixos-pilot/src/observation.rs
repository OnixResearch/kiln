use crate::{PilotProfile, Result};
use kiln_adapter_http_observe::{
    HttpObservationProfile, LiveEventBuffer, MemoryObservationSource, ObservationSnapshot,
};
use kiln_observe_core::EventWindow;
use std::path::Path;
use std::sync::Arc;

pub fn admit_loopback(profile: &HttpObservationProfile) -> Result<()> {
    if !matches!(profile.origin.host.as_str(), "localhost" | "127.0.0.1") {
        return Err("pilot observation requires an isolated loopback origin".into());
    }
    profile.admit().map_err(|error| error.to_string())
}

// r[impl kiln.onixos_pilot.observation]
pub fn serve(
    profile: &PilotProfile,
    journal_path: &Path,
    http: HttpObservationProfile,
) -> Result<()> {
    let (journal, state) = crate::coordinator::load_replayed(profile, journal_path)?;
    let snapshot = ObservationSnapshot::new(state).with_events(EventWindow {
        records: journal.records,
    });
    serve_snapshot(snapshot, http)
}

/// Serves one immutable canonical snapshot through the existing read-only port.
pub fn serve_snapshot(snapshot: ObservationSnapshot, http: HttpObservationProfile) -> Result<()> {
    admit_loopback(&http)?;
    // This process owns an immutable replay snapshot, never a control endpoint.
    let source = Arc::new(MemoryObservationSource::new(snapshot));
    let live = Arc::new(LiveEventBuffer::new(
        usize::try_from(http.live.max_records).map_err(|error| error.to_string())?,
    ));
    kiln_adapter_http_observe::serve(http, source, live).map_err(|error| error.to_string())
}
