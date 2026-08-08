use std::collections::BTreeMap;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{
    CommandEnvelope, Decision, Event, Metric, NotificationIntent, State, decide, evolve,
};
use kiln_core::operations::{LeaseState, OutboxRecord};
use kiln_core::provider::Operation;
use kiln_core::task_ops::{
    ControlKind, ControlRequest, CursorDecision, LogCursor, LogSegmentMeta, LogStreamMeta,
    ReadinessDecision, ReadinessInput, StatusRecord, StatusSelection, WaitPredicate, admit_control,
    decide_readiness, evaluate_wait, filter_status, validate_cursor,
};
use kiln_core::types::{
    AttemptId, CommandId, CoreResult, Diagnostic, EffectId, ExternalRef, Limits, ProviderProfileId,
    StreamId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitMode {
    Commit,
    InterruptBeforeCommit,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JournalSnapshot {
    pub state: State,
    pub events: Vec<Event>,
    pub outbox: Vec<OutboxRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryJournal {
    snapshot: JournalSnapshot,
}

impl MemoryJournal {
    #[must_use]
    pub fn snapshot(&self) -> JournalSnapshot {
        self.snapshot.clone()
    }

    #[must_use]
    pub fn recover(snapshot: JournalSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn transact(
        &mut self,
        command: &CommandEnvelope,
        mode: CommitMode,
    ) -> CoreResult<Decision> {
        let decision = decide(&self.snapshot.state, command, Limits::default())?;
        if mode == CommitMode::InterruptBeforeCommit {
            return Err(Diagnostic::new(
                "interrupted",
                "journal.commit",
                "transaction stopped before the atomic commit",
            ));
        }

        let mut next_state = self.snapshot.state.clone();
        for event in &decision.events {
            next_state = evolve(&next_state, event)?;
        }
        let mut next_outbox = self.snapshot.outbox.clone();
        for effect in &decision.effects {
            let (effect_id, profile) = effect_identity(effect);
            next_outbox.push(OutboxRecord {
                effect_id,
                provider_profile: profile,
                idempotency_key: ExternalRef::new("kiln", "command", command.command_id.as_str())?,
                lease: LeaseState::Unclaimed,
            });
        }
        let mut next_events = self.snapshot.events.clone();
        next_events.extend(decision.events.iter().cloned());
        self.snapshot = JournalSnapshot {
            state: next_state,
            events: next_events,
            outbox: next_outbox,
        };
        Ok(decision)
    }
}

fn effect_identity(effect: &kiln_core::domain::Effect) -> (EffectId, ProviderProfileId) {
    match effect {
        kiln_core::domain::Effect::Dispatch {
            effect_id,
            provider_profile,
            ..
        }
        | kiln_core::domain::Effect::Cancel {
            effect_id,
            provider_profile,
            ..
        }
        | kiln_core::domain::Effect::Reconcile {
            effect_id,
            provider_profile,
            ..
        } => (effect_id.clone(), provider_profile.clone()),
    }
}

pub fn run_reference_fixture(outcome: kiln_core::domain::Outcome) -> CoreResult<State> {
    let run_id = kiln_core::types::RunId::new("reference-run")?;
    let attempt_id = kiln_core::types::AttemptId::new("reference-attempt")?;
    let provider_profile = ProviderProfileId::new("standalone-build-v1")?;
    let mut journal = MemoryJournal::default();
    let start = CommandEnvelope {
        contract_version: kiln_core::types::CONTRACT_VERSION_V1,
        command_id: kiln_core::types::CommandId::new("reference-start")?,
        expected_revision: journal.snapshot.state.revision,
        command: kiln_core::domain::Command::StartRun {
            run_id,
            attempt_id: attempt_id.clone(),
            provider_profile,
            effect_id: EffectId::new("reference-dispatch")?,
        },
    };
    journal.transact(&start, CommitMode::Commit)?;
    let completion = CommandEnvelope {
        contract_version: kiln_core::types::CONTRACT_VERSION_V1,
        command_id: kiln_core::types::CommandId::new("reference-complete")?,
        expected_revision: journal.snapshot.state.revision,
        command: kiln_core::domain::Command::CompleteAttempt {
            attempt_id,
            outcome,
            external_reference: None,
            products: Vec::new(),
            metrics: Vec::new(),
        },
    };
    journal.transact(&completion, CommitMode::Commit)?;
    Ok(journal.snapshot.state)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaitResult {
    Matched(Vec<StatusRecord>),
    ClientTimedOut(Vec<StatusRecord>),
    Pending,
}

#[must_use]
pub fn wait_once(
    snapshot: &[StatusRecord],
    predicate: WaitPredicate,
    client_deadline_reached: bool,
) -> WaitResult {
    if evaluate_wait(snapshot, predicate) {
        return WaitResult::Matched(snapshot.to_vec());
    }
    if client_deadline_reached {
        return WaitResult::ClientTimedOut(snapshot.to_vec());
    }
    WaitResult::Pending
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusFormat {
    Human,
    MachineV1,
}

pub fn render_status(
    records: &[StatusRecord],
    selection: &StatusSelection,
    max_records: usize,
    format: StatusFormat,
) -> CoreResult<String> {
    let selected = filter_status(records, selection, max_records)?;
    match format {
        StatusFormat::MachineV1 => serde_json::to_string(&selected).map_err(|_| {
            Diagnostic::new(
                "status_encoding",
                "status.records",
                "versioned machine status encoding failed",
            )
        }),
        StatusFormat::Human => {
            let mut lines = Vec::with_capacity(selected.len());
            for record in selected {
                lines.push(format!(
                    "{} {} {:?} {:?}",
                    record.run_id.as_str(),
                    record.task_id.as_str(),
                    record.execution,
                    record.outcome
                ));
            }
            Ok(lines.join("\n"))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationsProjection {
    pub source_revision: u64,
    pub metrics: Vec<Metric>,
    pub notifications: Vec<NotificationIntent>,
}

pub fn rebuild_operations_projection(
    state: &State,
    max_records: usize,
) -> CoreResult<OperationsProjection> {
    let mut metrics = Vec::new();
    let mut notifications = Vec::new();
    for (attempt_id, attempt) in &state.attempts {
        for metric in &attempt.metrics {
            if &metric.producing_attempt != attempt_id {
                return Err(Diagnostic::new(
                    "projection_metric_link",
                    "projection.metrics",
                    "metric does not link to its producing attempt",
                ));
            }
            metrics.push(metric.clone());
        }
    }
    for (run_id, run) in &state.runs {
        if run.outcome.is_terminal() {
            notifications.push(NotificationIntent {
                run_id: run_id.clone(),
                outcome: run.outcome,
            });
        }
    }
    if metrics.len() > max_records || notifications.len() > max_records {
        return Err(Diagnostic::new(
            "projection_bound",
            "projection.records",
            "operations projection exceeds its named record bound",
        ));
    }
    Ok(OperationsProjection {
        source_revision: state.revision,
        metrics,
        notifications,
    })
}

pub fn page_records<T: Clone>(records: &[T], offset: usize, limit: usize) -> CoreResult<Vec<T>> {
    if limit == 0 || offset > records.len() {
        return Err(Diagnostic::new(
            "pagination_bound",
            "projection.pagination",
            "projection pagination offset or limit is invalid",
        ));
    }
    let end = offset.saturating_add(limit).min(records.len());
    records
        .get(offset..end)
        .map(|slice| slice.to_vec())
        .ok_or(Diagnostic::new(
            "pagination_range",
            "projection.pagination",
            "projection pagination range is invalid",
        ))
}

pub fn schedule_readiness(input: &ReadinessInput) -> CoreResult<bool> {
    match decide_readiness(input) {
        ReadinessDecision::Ready => Ok(true),
        ReadinessDecision::Blocked(_) => Ok(false),
        ReadinessDecision::Denied(diagnostic) => Err(diagnostic),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlDispatch {
    Canonical(CommandEnvelope),
    Provider {
        operation: Operation,
        attempt_id: AttemptId,
    },
}

pub fn wire_control(
    request: &ControlRequest,
    command_id: CommandId,
    expected_revision: u64,
    scope: ExternalRef,
) -> CoreResult<ControlDispatch> {
    match admit_control(request)? {
        None => Ok(ControlDispatch::Canonical(CommandEnvelope {
            contract_version: kiln_core::types::CONTRACT_VERSION_V1,
            command_id,
            expected_revision,
            command: kiln_core::domain::Command::SetHold {
                scope,
                held: request.kind == ControlKind::Hold,
            },
        })),
        Some(operation) => {
            let attempt_id = request.attempt_id.clone().ok_or(Diagnostic::new(
                "missing_attempt",
                "control.attempt_id",
                "provider control has no exact attempt",
            ))?;
            Ok(ControlDispatch::Provider {
                operation,
                attempt_id,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredLogStream {
    pub metadata: LogStreamMeta,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct LogStore {
    streams: BTreeMap<StreamId, StoredLogStream>,
}

impl LogStore {
    pub fn recover(
        streams: Vec<StoredLogStream>,
        max_streams: usize,
        max_segments_per_stream: usize,
        max_total_bytes: usize,
    ) -> CoreResult<Self> {
        if streams.len() > max_streams {
            return Err(Diagnostic::new(
                "log_stream_bound",
                "log.streams",
                "recovered log stream count exceeds its named bound",
            ));
        }
        let mut recovered = Self::default();
        for stream in streams {
            validate_stored_stream(&stream, max_segments_per_stream, max_total_bytes)?;
            if recovered
                .streams
                .insert(stream.metadata.stream_id.clone(), stream)
                .is_some()
            {
                return Err(Diagnostic::new(
                    "duplicate_log_stream",
                    "log.stream_id",
                    "recovery found duplicate log stream metadata",
                ));
            }
        }
        Ok(recovered)
    }

    pub fn create(&mut self, metadata: LogStreamMeta) -> CoreResult<()> {
        if self.streams.contains_key(&metadata.stream_id) {
            return Err(Diagnostic::new(
                "stream_exists",
                "log.stream_id",
                "log stream already exists",
            ));
        }
        self.streams.insert(
            metadata.stream_id.clone(),
            StoredLogStream {
                metadata,
                bytes: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn append(
        &mut self,
        stream_id: &StreamId,
        bytes: &[u8],
        max_total_bytes: usize,
    ) -> CoreResult<LogCursor> {
        let stream = self.streams.get_mut(stream_id).ok_or(Diagnostic::new(
            "missing_stream",
            "log.stream_id",
            "log stream does not exist",
        ))?;
        let next_len = stream
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Diagnostic::new(
                "log_bound",
                "log.bytes",
                "log byte count overflowed",
            ))?;
        if next_len > max_total_bytes {
            stream.metadata.truncated = true;
            return Err(Diagnostic::new(
                "log_bound",
                "log.bytes",
                "log stream exceeds its named total-byte bound",
            ));
        }
        let start_offset = u64::try_from(stream.bytes.len()).map_err(|_| {
            Diagnostic::new(
                "log_offset",
                "log.bytes",
                "log offset cannot be represented",
            )
        })?;
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            Diagnostic::new(
                "log_offset",
                "log.bytes",
                "log segment length cannot be represented",
            )
        })?;
        let identity = identity_from_bytes(bytes).map_err(|_| {
            Diagnostic::new("log_identity", "log.bytes", "log segment identity failed")
        })?;
        stream.bytes.extend_from_slice(bytes);
        stream.metadata.segments.push(LogSegmentMeta {
            identity,
            start_offset,
            byte_len,
        });
        stream.metadata.retained_end = start_offset.saturating_add(byte_len);
        Ok(LogCursor {
            stream_id: stream_id.clone(),
            offset: stream.metadata.retained_end,
        })
    }

    pub fn append_redacted(
        &mut self,
        stream_id: &StreamId,
        bytes: &[u8],
        secrets: &[&str],
        marker: &str,
        max_total_bytes: usize,
    ) -> CoreResult<LogCursor> {
        let text = core::str::from_utf8(bytes).map_err(|_| {
            Diagnostic::new(
                "log_encoding",
                "log.bytes",
                "redacted log input must be valid UTF-8",
            )
        })?;
        let redacted =
            kiln_core::task_ops::safe_diagnostic(text, secrets, marker, max_total_bytes)?;
        self.append(stream_id, redacted.as_bytes(), max_total_bytes)
    }

    pub fn follow(
        &self,
        cursor: &LogCursor,
        max_read_bytes: usize,
    ) -> CoreResult<(Vec<u8>, LogCursor, bool)> {
        let stream = self.streams.get(&cursor.stream_id).ok_or(Diagnostic::new(
            "missing_stream",
            "log_cursor.stream_id",
            "log stream does not exist",
        ))?;
        match validate_cursor(&stream.metadata, cursor)? {
            CursorDecision::Gap { .. } => {
                return Err(Diagnostic::new(
                    "cursor_gap",
                    "log_cursor.offset",
                    "cursor does not name retained log data",
                ));
            }
            CursorDecision::Complete => {
                return Ok((Vec::new(), cursor.clone(), true));
            }
            CursorDecision::Valid => {}
        }
        let bytes = self.read(cursor, max_read_bytes)?;
        let byte_count = u64::try_from(bytes.len()).map_err(|_| {
            Diagnostic::new(
                "cursor_offset",
                "log.bytes",
                "follow byte count cannot be represented",
            )
        })?;
        let next = LogCursor {
            stream_id: cursor.stream_id.clone(),
            offset: cursor.offset.saturating_add(byte_count),
        };
        let complete = stream.metadata.complete && next.offset == stream.metadata.retained_end;
        Ok((bytes, next, complete))
    }

    pub fn mark_complete(&mut self, stream_id: &StreamId) -> CoreResult<()> {
        let stream = self.streams.get_mut(stream_id).ok_or(Diagnostic::new(
            "missing_stream",
            "log.stream_id",
            "log stream does not exist",
        ))?;
        stream.metadata.complete = true;
        Ok(())
    }

    pub fn expire_before(&mut self, stream_id: &StreamId, retained_start: u64) -> CoreResult<()> {
        let stream = self.streams.get_mut(stream_id).ok_or(Diagnostic::new(
            "missing_stream",
            "log.stream_id",
            "log stream does not exist",
        ))?;
        if retained_start < stream.metadata.retained_start
            || retained_start > stream.metadata.retained_end
        {
            return Err(Diagnostic::new(
                "invalid_retention",
                "log.retained_start",
                "retention boundary is outside the committed log range",
            ));
        }
        stream.metadata.retained_start = retained_start;
        Ok(())
    }

    pub fn read(&self, cursor: &LogCursor, max_read_bytes: usize) -> CoreResult<Vec<u8>> {
        let stream = self.streams.get(&cursor.stream_id).ok_or(Diagnostic::new(
            "missing_stream",
            "log_cursor.stream_id",
            "log stream does not exist",
        ))?;
        let start = usize::try_from(cursor.offset).map_err(|_| {
            Diagnostic::new(
                "cursor_offset",
                "log_cursor.offset",
                "cursor offset cannot be represented",
            )
        })?;
        if start < usize::try_from(stream.metadata.retained_start).unwrap_or(usize::MAX)
            || start > stream.bytes.len()
        {
            return Err(Diagnostic::new(
                "cursor_gap",
                "log_cursor.offset",
                "cursor does not name retained log data",
            ));
        }
        let end = start.saturating_add(max_read_bytes).min(stream.bytes.len());
        let Some(slice) = stream.bytes.get(start..end) else {
            return Err(Diagnostic::new(
                "cursor_range",
                "log_cursor.offset",
                "log cursor range is invalid",
            ));
        };
        Ok(slice.to_vec())
    }
}

fn validate_stored_stream(
    stream: &StoredLogStream,
    max_segments_per_stream: usize,
    max_total_bytes: usize,
) -> CoreResult<()> {
    if stream.metadata.segments.len() > max_segments_per_stream
        || stream.bytes.len() > max_total_bytes
    {
        return Err(Diagnostic::new(
            "log_recovery_bound",
            "log.stream",
            "recovered log stream exceeds a named bound",
        ));
    }
    let byte_len = u64::try_from(stream.bytes.len()).map_err(|_| {
        Diagnostic::new(
            "log_offset",
            "log.bytes",
            "recovered log byte count cannot be represented",
        )
    })?;
    if stream.metadata.retained_end != byte_len
        || stream.metadata.retained_start > stream.metadata.retained_end
    {
        return Err(Diagnostic::new(
            "log_metadata_mismatch",
            "log.metadata",
            "log metadata does not match committed bytes",
        ));
    }
    let mut expected_offset = 0_u64;
    for segment in &stream.metadata.segments {
        if segment.start_offset != expected_offset {
            return Err(Diagnostic::new(
                "log_segment_gap",
                "log.segments",
                "log segments are missing, overlapping, or out of order",
            ));
        }
        let end = segment
            .start_offset
            .checked_add(segment.byte_len)
            .ok_or(Diagnostic::new(
                "log_offset",
                "log.segments",
                "log segment end overflowed",
            ))?;
        let start_index = usize::try_from(segment.start_offset).map_err(|_| {
            Diagnostic::new(
                "log_offset",
                "log.segments",
                "segment offset cannot be represented",
            )
        })?;
        let end_index = usize::try_from(end).map_err(|_| {
            Diagnostic::new(
                "log_offset",
                "log.segments",
                "segment end cannot be represented",
            )
        })?;
        let bytes = stream
            .bytes
            .get(start_index..end_index)
            .ok_or(Diagnostic::new(
                "partial_log_write",
                "log.segments",
                "log segment metadata names missing bytes",
            ))?;
        if identity_from_bytes(bytes).map_err(|_| {
            Diagnostic::new("log_identity", "log.segments", "segment identity failed")
        })? != segment.identity
        {
            return Err(Diagnostic::new(
                "log_identity_mismatch",
                "log.segments",
                "log segment bytes do not match their BLAKE3 identity",
            ));
        }
        expected_offset = end;
    }
    if expected_offset != byte_len {
        return Err(Diagnostic::new(
            "partial_log_metadata",
            "log.segments",
            "committed log bytes have no atomic segment metadata",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kiln_core::domain::{Command, Outcome};
    use kiln_core::types::{AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, RunId};

    use super::*;

    fn start() -> CommandEnvelope {
        CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new("start").unwrap_or_else(|_| unreachable!()),
            expected_revision: 0,
            command: Command::StartRun {
                run_id: RunId::new("run").unwrap_or_else(|_| unreachable!()),
                attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
                provider_profile: ProviderProfileId::new("standalone")
                    .unwrap_or_else(|_| unreachable!()),
                effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            },
        }
    }

    #[test]
    fn event_and_outbox_commit_together() {
        let mut journal = MemoryJournal::default();
        assert!(journal.transact(&start(), CommitMode::Commit).is_ok());
        let snapshot = journal.snapshot();
        assert_eq!(snapshot.events.len(), snapshot.outbox.len());
        assert_eq!(snapshot.state.runs.len(), 1);
    }

    #[test]
    fn interruption_commits_nothing() {
        let mut journal = MemoryJournal::default();
        assert!(
            journal
                .transact(&start(), CommitMode::InterruptBeforeCommit)
                .is_err()
        );
        assert_eq!(journal.snapshot(), JournalSnapshot::default());
    }

    #[test]
    fn unknown_outcome_survives_recovery() {
        let mut journal = MemoryJournal::default();
        assert!(journal.transact(&start(), CommitMode::Commit).is_ok());
        let command = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new("unknown").unwrap_or_else(|_| unreachable!()),
            expected_revision: journal.snapshot().state.revision,
            command: Command::RecordUnknown {
                attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
                external_reference: None,
            },
        };
        assert!(journal.transact(&command, CommitMode::Commit).is_ok());
        let recovered = MemoryJournal::recover(journal.snapshot());
        let outcome = recovered
            .snapshot()
            .state
            .attempts
            .values()
            .next()
            .map(|attempt| attempt.outcome);
        assert_eq!(outcome, Some(Outcome::Unknown));
    }

    #[test]
    fn reference_profile_completes_accepted_and_rejected_runs() {
        let accepted = run_reference_fixture(Outcome::Success);
        let rejected = run_reference_fixture(Outcome::DomainFailure);
        assert_eq!(
            accepted
                .as_ref()
                .ok()
                .and_then(|state| state.runs.values().next())
                .map(|run| run.outcome),
            Some(Outcome::Success)
        );
        assert_eq!(
            rejected
                .as_ref()
                .ok()
                .and_then(|state| state.runs.values().next())
                .map(|run| run.outcome),
            Some(Outcome::DomainFailure)
        );
    }

    #[test]
    fn bounded_log_survives_store_clone() {
        const MAX_LOG_BYTES: usize = 64;
        const MAX_READ_BYTES: usize = 64;
        const EXPECTED_OUTPUT_BYTES: u64 = 6;
        let stream_id = StreamId::new("stream").unwrap_or_else(|_| unreachable!());
        let metadata = LogStreamMeta {
            stream_id: stream_id.clone(),
            attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
            segments: Vec::new(),
            retained_start: 0,
            retained_end: 0,
            complete: false,
            truncated: false,
        };
        let mut store = LogStore::default();
        assert!(store.create(metadata).is_ok());
        assert!(store.append(&stream_id, b"output", MAX_LOG_BYTES).is_ok());
        let recovered = store.clone();
        let cursor = LogCursor {
            stream_id,
            offset: 0,
        };
        assert_eq!(
            recovered.read(&cursor, MAX_READ_BYTES),
            Ok(b"output".to_vec())
        );
        let followed = recovered.follow(&cursor, MAX_READ_BYTES);
        assert_eq!(
            followed.map(|(bytes, next, _complete)| (bytes, next.offset)),
            Ok((b"output".to_vec(), EXPECTED_OUTPUT_BYTES))
        );
    }

    #[test]
    fn client_deadline_does_not_change_attempt_outcome() {
        let result = wait_once(&[], WaitPredicate::AllTerminal, true);
        assert_eq!(result, WaitResult::ClientTimedOut(Vec::new()));
    }

    #[test]
    fn hold_wires_to_canonical_command_without_provider_effect() {
        let request = ControlRequest {
            kind: ControlKind::Hold,
            attempt_id: None,
            active_attempt: false,
            declared_operations: std::collections::BTreeSet::new(),
        };
        let dispatch = wire_control(
            &request,
            CommandId::new("hold").unwrap_or_else(|_| unreachable!()),
            0,
            ExternalRef::new("kiln", "task", "task").unwrap_or_else(|_| unreachable!()),
        );
        assert!(matches!(dispatch, Ok(ControlDispatch::Canonical(_))));
    }

    #[test]
    fn recovery_rejects_partial_log_metadata() {
        const MAX_STREAMS: usize = 4;
        const MAX_SEGMENTS: usize = 4;
        const MAX_BYTES: usize = 64;
        let stream_id = StreamId::new("stream").unwrap_or_else(|_| unreachable!());
        let stream = StoredLogStream {
            metadata: LogStreamMeta {
                stream_id,
                attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
                segments: Vec::new(),
                retained_start: 0,
                retained_end: 1,
                complete: false,
                truncated: false,
            },
            bytes: vec![b'x'],
        };
        assert!(LogStore::recover(vec![stream], MAX_STREAMS, MAX_SEGMENTS, MAX_BYTES).is_err());
    }

    #[test]
    fn machine_status_output_is_versioned_and_bounded() {
        let selection = StatusSelection {
            project_ids: std::collections::BTreeSet::new(),
            evaluation_ids: std::collections::BTreeSet::new(),
            run_ids: std::collections::BTreeSet::new(),
            task_ids: std::collections::BTreeSet::new(),
            attempt_ids: std::collections::BTreeSet::new(),
            providers: std::collections::BTreeSet::new(),
            capacity_classes: std::collections::BTreeSet::new(),
            outcomes: std::collections::BTreeSet::new(),
        };
        assert_eq!(
            render_status(&[], &selection, 1, StatusFormat::MachineV1),
            Ok("[]".to_string())
        );
    }
}
