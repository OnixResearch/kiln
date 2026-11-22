use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::domain::{Outcome, State};
use crate::provider::Operation;
use crate::types::{
    Alias, AttemptId, ContentId, CoreResult, Diagnostic, EvaluationId, ProjectId,
    ProviderProfileId, RunId, StreamId, TaskId,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ReadinessFacet {
    Held,
    Delayed,
    DependencyBlocked,
    CapacityBlocked,
    Ready,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ExecutionFacet {
    NotDispatched,
    Dispatching,
    Running,
    ProviderPaused,
    NotObserved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StatusRecord {
    pub project_id: ProjectId,
    pub evaluation_id: EvaluationId,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub attempt_id: Option<AttemptId>,
    pub provider: ProviderProfileId,
    pub capacity_class: Alias,
    pub readiness: ReadinessFacet,
    pub execution: ExecutionFacet,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StatusSelection {
    pub project_ids: BTreeSet<ProjectId>,
    pub evaluation_ids: BTreeSet<EvaluationId>,
    pub run_ids: BTreeSet<RunId>,
    pub task_ids: BTreeSet<TaskId>,
    pub attempt_ids: BTreeSet<AttemptId>,
    pub providers: BTreeSet<ProviderProfileId>,
    pub capacity_classes: BTreeSet<Alias>,
    pub outcomes: BTreeSet<Outcome>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StatusContext {
    pub project_id: ProjectId,
    pub evaluation_id: EvaluationId,
    pub task_id: TaskId,
    pub provider: ProviderProfileId,
    pub capacity_class: Alias,
    pub readiness: ReadinessFacet,
    pub execution: ExecutionFacet,
}

pub fn project_attempt_status(
    state: &State,
    run_id: &RunId,
    attempt_id: &AttemptId,
    context: StatusContext,
) -> CoreResult<StatusRecord> {
    let run = state.runs.get(run_id).ok_or(Diagnostic::new(
        "missing_run",
        "status.run_id",
        "status projection names a missing run",
    ))?;
    let attempt = state.attempts.get(attempt_id).ok_or(Diagnostic::new(
        "missing_attempt",
        "status.attempt_id",
        "status projection names a missing attempt",
    ))?;
    if run.active_attempt != *attempt_id
        || attempt.run_id != *run_id
        || attempt.provider_profile != context.provider
    {
        return Err(Diagnostic::new(
            "status_link_mismatch",
            "status.attempt_id",
            "status projection attempt does not match the canonical run or provider",
        ));
    }
    Ok(StatusRecord {
        project_id: context.project_id,
        evaluation_id: context.evaluation_id,
        run_id: run_id.clone(),
        task_id: context.task_id,
        attempt_id: Some(attempt_id.clone()),
        provider: context.provider,
        capacity_class: context.capacity_class,
        readiness: context.readiness,
        execution: context.execution,
        outcome: attempt.outcome,
    })
}

pub fn filter_status(
    records: &[StatusRecord],
    selection: &StatusSelection,
    max_records: usize,
) -> CoreResult<Vec<StatusRecord>> {
    if records.len() > max_records {
        return Err(Diagnostic::new(
            "status_bound",
            "status.records",
            "status record count exceeds its named bound",
        ));
    }
    let mut selected: Vec<StatusRecord> = records
        .iter()
        .filter(|record| {
            (selection.project_ids.is_empty() || selection.project_ids.contains(&record.project_id))
                && (selection.evaluation_ids.is_empty()
                    || selection.evaluation_ids.contains(&record.evaluation_id))
                && (selection.run_ids.is_empty() || selection.run_ids.contains(&record.run_id))
                && (selection.task_ids.is_empty() || selection.task_ids.contains(&record.task_id))
                && (selection.attempt_ids.is_empty()
                    || record
                        .attempt_id
                        .as_ref()
                        .is_some_and(|attempt| selection.attempt_ids.contains(attempt)))
                && (selection.providers.is_empty()
                    || selection.providers.contains(&record.provider))
                && (selection.capacity_classes.is_empty()
                    || selection.capacity_classes.contains(&record.capacity_class))
                && (selection.outcomes.is_empty() || selection.outcomes.contains(&record.outcome))
        })
        .cloned()
        .collect();
    selected.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then(left.evaluation_id.cmp(&right.evaluation_id))
            .then(left.run_id.cmp(&right.run_id))
            .then(left.task_id.cmp(&right.task_id))
            .then(left.attempt_id.cmp(&right.attempt_id))
    });
    Ok(selected)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DependencyOutcome {
    Success,
    Failure,
    Pending,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DependencyFact {
    pub task_id: TaskId,
    pub outcome: DependencyOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapacityObservation {
    pub provider: ProviderProfileId,
    pub class: Alias,
    pub in_flight: usize,
    pub limit: usize,
    pub observed_at: u64,
    pub max_age: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadinessInput {
    pub task_id: TaskId,
    pub terminal: bool,
    pub dependencies: Vec<DependencyFact>,
    pub held: bool,
    pub not_before: Option<u64>,
    pub observed_time: Option<u64>,
    pub capacity: Option<CapacityObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReadinessDecision {
    Ready,
    Blocked(BTreeSet<ReadinessFacet>),
    Denied(Diagnostic),
}

pub fn decide_readiness(input: &ReadinessInput) -> ReadinessDecision {
    if input.terminal {
        return ReadinessDecision::Blocked(BTreeSet::from([ReadinessFacet::NotApplicable]));
    }
    if input
        .dependencies
        .iter()
        .any(|fact| fact.outcome == DependencyOutcome::Failure)
    {
        return ReadinessDecision::Denied(Diagnostic::new(
            "dependency_failed",
            "readiness.dependencies",
            "a required dependency failed",
        ));
    }
    let mut blocked = BTreeSet::new();
    if input
        .dependencies
        .iter()
        .any(|fact| fact.outcome == DependencyOutcome::Pending)
    {
        blocked.insert(ReadinessFacet::DependencyBlocked);
    }
    if input.held {
        blocked.insert(ReadinessFacet::Held);
    }
    if let Some(not_before) = input.not_before {
        let Some(observed_time) = input.observed_time else {
            return ReadinessDecision::Denied(Diagnostic::new(
                "missing_time",
                "readiness.observed_time",
                "delayed task requires an admitted time observation",
            ));
        };
        if observed_time < not_before {
            blocked.insert(ReadinessFacet::Delayed);
        }
    }
    if let Some(capacity) = &input.capacity {
        if capacity.limit == 0 {
            return ReadinessDecision::Denied(Diagnostic::new(
                "invalid_capacity_limit",
                "capacity.limit",
                "capacity limit must be positive",
            ));
        }
        let Some(now) = input.observed_time else {
            return ReadinessDecision::Denied(Diagnostic::new(
                "missing_time",
                "readiness.observed_time",
                "capacity freshness requires an admitted time observation",
            ));
        };
        if now.saturating_sub(capacity.observed_at) > capacity.max_age {
            return ReadinessDecision::Denied(Diagnostic::new(
                "stale_capacity",
                "capacity.observed_at",
                "capacity observation is stale",
            ));
        }
        if capacity.in_flight >= capacity.limit {
            blocked.insert(ReadinessFacet::CapacityBlocked);
        }
    } else {
        return ReadinessDecision::Denied(Diagnostic::new(
            "missing_capacity",
            "readiness.capacity",
            "provider-local capacity observation is missing",
        ));
    }
    if blocked.is_empty() {
        ReadinessDecision::Ready
    } else {
        ReadinessDecision::Blocked(blocked)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogSegmentMeta {
    pub identity: ContentId,
    pub start_offset: u64,
    pub byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogStreamMeta {
    pub stream_id: StreamId,
    pub attempt_id: AttemptId,
    pub segments: Vec<LogSegmentMeta>,
    pub retained_start: u64,
    pub retained_end: u64,
    pub complete: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogCursor {
    pub stream_id: StreamId,
    pub offset: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CursorDecision {
    Valid,
    Gap { retained_start: u64 },
    Complete,
}

pub fn validate_cursor(meta: &LogStreamMeta, cursor: &LogCursor) -> CoreResult<CursorDecision> {
    if meta.stream_id != cursor.stream_id {
        return Err(Diagnostic::new(
            "stream_mismatch",
            "log_cursor.stream_id",
            "cursor belongs to another log stream",
        ));
    }
    if cursor.offset < meta.retained_start {
        return Ok(CursorDecision::Gap {
            retained_start: meta.retained_start,
        });
    }
    if cursor.offset > meta.retained_end {
        return Err(Diagnostic::new(
            "cursor_ahead",
            "log_cursor.offset",
            "cursor is ahead of retained log data",
        ));
    }
    if cursor.offset == meta.retained_end && meta.complete {
        return Ok(CursorDecision::Complete);
    }
    Ok(CursorDecision::Valid)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WaitPredicate {
    AllTerminal,
    AnyTerminal,
    AllSuccessful,
}

pub fn evaluate_wait(records: &[StatusRecord], predicate: WaitPredicate) -> bool {
    if records.is_empty() {
        return false;
    }
    match predicate {
        WaitPredicate::AllTerminal => records.iter().all(|record| record.outcome.is_terminal()),
        WaitPredicate::AnyTerminal => records.iter().any(|record| record.outcome.is_terminal()),
        WaitPredicate::AllSuccessful => records
            .iter()
            .all(|record| record.outcome == Outcome::Success),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ControlKind {
    Hold,
    Release,
    Cancel,
    Pause,
    Resume,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ControlRequest {
    pub kind: ControlKind,
    pub attempt_id: Option<AttemptId>,
    pub active_attempt: bool,
    pub declared_operations: BTreeSet<Operation>,
}

pub fn admit_control(request: &ControlRequest) -> CoreResult<Option<Operation>> {
    let operation = match request.kind {
        ControlKind::Hold | ControlKind::Release => return Ok(None),
        ControlKind::Cancel => Operation::Cancel,
        ControlKind::Pause => Operation::Pause,
        ControlKind::Resume => Operation::Resume,
    };
    if request.attempt_id.is_none() || !request.active_attempt {
        return Err(Diagnostic::new(
            "stale_attempt",
            "control.attempt_id",
            "provider control requires an exact active attempt",
        ));
    }
    if !request.declared_operations.contains(&operation) {
        return Err(Diagnostic::new(
            "unsupported_control",
            "control.kind",
            "provider does not declare this control capability",
        ));
    }
    Ok(Some(operation))
}

pub fn safe_diagnostic(
    public_text: &str,
    secrets: &[&str],
    marker: &str,
    max_bytes: usize,
) -> CoreResult<String> {
    let mut rendered = public_text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            rendered = rendered.replace(secret, marker);
        }
    }
    if rendered.len() > max_bytes {
        return Err(Diagnostic::new(
            "diagnostic_bound",
            "diagnostic",
            "diagnostic exceeds its named byte bound",
        ));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 10;
    const CAPACITY_LIMIT: usize = 2;
    const MAX_AGE: u64 = 5;
    const DIAGNOSTIC_BOUND: usize = 128;

    fn readiness() -> ReadinessInput {
        ReadinessInput {
            task_id: TaskId::new("task").unwrap_or_else(|_| unreachable!()),
            terminal: false,
            dependencies: Vec::new(),
            held: false,
            not_before: Some(NOW),
            observed_time: Some(NOW),
            capacity: Some(CapacityObservation {
                provider: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
                class: Alias::new("local").unwrap_or_else(|_| unreachable!()),
                in_flight: 0,
                limit: CAPACITY_LIMIT,
                observed_at: NOW,
                max_age: MAX_AGE,
            }),
        }
    }

    #[test]
    fn admits_ready_task() {
        assert_eq!(decide_readiness(&readiness()), ReadinessDecision::Ready);
    }

    #[test]
    fn denies_delayed_task_without_time() {
        let mut input = readiness();
        input.observed_time = None;
        assert!(matches!(
            decide_readiness(&input),
            ReadinessDecision::Denied(_)
        ));
    }

    #[test]
    fn redacts_secret_value() {
        let rendered = safe_diagnostic("token=secret", &["secret"], "[redacted]", DIAGNOSTIC_BOUND);
        assert_eq!(rendered, Ok("token=[redacted]".to_string()));
    }
}
