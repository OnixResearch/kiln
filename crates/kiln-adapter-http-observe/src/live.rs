use std::collections::VecDeque;
use std::sync::Mutex;

use kiln_core::operations::JournalRecord;
use kiln_observe_core::ObservationView;
use serde::Serialize;

use crate::{ObserveError, ObserveResult};

#[derive(Debug)]
struct LiveState {
    records: VecDeque<JournalRecord>,
    dropped: u64,
}

#[derive(Debug)]
pub struct LiveEventBuffer {
    state: Mutex<LiveState>,
    max_records: usize,
}

impl LiveEventBuffer {
    #[must_use]
    pub fn new(max_records: usize) -> Self {
        Self {
            state: Mutex::new(LiveState {
                records: VecDeque::new(),
                dropped: 0,
            }),
            max_records,
        }
    }

    // r[impl kiln.observe.http.live]
    pub fn push(&self, record: JournalRecord) -> ObserveResult<()> {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(last) = state.records.back() {
            let expected = last.expected_revision.saturating_add(1);
            if record.expected_revision != expected {
                return Err(ObserveError::new(
                    "live_revision_gap",
                    "live event does not continue the committed revision sequence",
                ));
            }
        }
        if state.records.len() >= self.max_records {
            let _ = state.records.pop_front();
            state.dropped = state.dropped.saturating_add(1);
        }
        state.records.push_back(record);
        Ok(())
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        match self.state.lock() {
            Ok(state) => state.dropped,
            Err(poisoned) => poisoned.into_inner().dropped,
        }
    }

    #[must_use]
    pub fn snapshot_after(&self, after: Option<u64>, limit: usize) -> LiveSnapshot {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let records: Vec<JournalRecord> = state
            .records
            .iter()
            .filter(|record| after.is_none_or(|after| record.expected_revision > after))
            .take(limit)
            .cloned()
            .collect();
        LiveSnapshot {
            records,
            dropped: state.dropped,
            oldest_revision: state.records.front().map(|record| record.expected_revision),
            latest_revision: state.records.back().map(|record| record.expected_revision),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSnapshot {
    pub records: Vec<JournalRecord>,
    pub dropped: u64,
    pub oldest_revision: Option<u64>,
    pub latest_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveFrame {
    View {
        view: Box<ObservationView>,
    },
    Lag {
        dropped: u64,
        oldest_revision: Option<u64>,
        latest_revision: Option<u64>,
        state_revision: u64,
    },
}

impl LiveFrame {
    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        match self {
            Self::View { .. } => "observe",
            Self::Lag { .. } => "lag",
        }
    }
}

// r[impl kiln.observe.http.live.scenario.loss]
pub fn sse_frame(frame: &LiveFrame, max_bytes: usize, id: Option<u64>) -> ObserveResult<Vec<u8>> {
    let body = serde_json::to_vec(frame)
        .map_err(|_| ObserveError::new("live_encoding", "live frame could not be encoded"))?;
    if body.len() > max_bytes {
        return Err(ObserveError::new(
            "live_encoding_bound",
            "encoded live frame exceeds its named byte bound",
        ));
    }
    let mut output = Vec::new();
    if let Some(id) = id {
        output.extend_from_slice(b"id: ");
        output.extend_from_slice(id.to_string().as_bytes());
        output.push(b'\n');
    }
    output.extend_from_slice(b"event: ");
    output.extend_from_slice(frame.event_name().as_bytes());
    output.extend_from_slice(b"\ndata: ");
    output.extend_from_slice(&body);
    output.extend_from_slice(b"\n\n");
    Ok(output)
}
