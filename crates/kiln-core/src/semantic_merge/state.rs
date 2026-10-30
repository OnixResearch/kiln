use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::identity::{
    ArtifactKind, AttemptBinding, BindingComponent, RequestBinding, artifact_binding,
    attempt_binding, require_binding,
};
use super::{
    AdmittedRequest, CheckMode, CheckerIdentity, MAX_INDEPENDENT_GATES, RetryPolicy,
    RevalidationObligation, strictly_sorted,
};
use crate::types::{Alias, ContentId, CoreResult, Diagnostic};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExhaustionBound {
    WallTime,
    Memory,
    Output,
    Steps,
}

/// What a checker producer reports. Staleness is Kiln's judgement, never the producer's.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckerVerdict {
    Verified,
    Counterexample,
    Unknown,
    Unsupported,
    Exhausted(ExhaustionBound),
    Cancelled,
    ToolError,
}

/// The full semantic merge outcome lattice that Kiln preserves.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticOutcome {
    Verified,
    Counterexample,
    Unknown,
    Unsupported,
    Exhausted(ExhaustionBound),
    Cancelled,
    Stale(BindingComponent),
    ToolError,
}

impl SemanticOutcome {
    /// Only a counterexample demonstrates a conflict; unknown never does.
    #[must_use]
    pub const fn conflict_demonstrated(self) -> bool {
        matches!(self, Self::Counterexample)
    }

    /// Outcomes that the same checker reproduces for the same binding and may be cached.
    #[must_use]
    pub const fn is_deterministic(self) -> bool {
        matches!(
            self,
            Self::Verified | Self::Counterexample | Self::Unsupported
        )
    }

    const fn from_verdict(verdict: CheckerVerdict) -> Self {
        match verdict {
            CheckerVerdict::Verified => Self::Verified,
            CheckerVerdict::Counterexample => Self::Counterexample,
            CheckerVerdict::Unknown => Self::Unknown,
            CheckerVerdict::Unsupported => Self::Unsupported,
            CheckerVerdict::Exhausted(bound) => Self::Exhausted(bound),
            CheckerVerdict::Cancelled => Self::Cancelled,
            CheckerVerdict::ToolError => Self::ToolError,
        }
    }
}

/// Evidence kinds a producer may claim. Only a semantic checker receipt is admissible.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    SemanticCheckerReceipt,
    CleanTextMerge,
    EqualSourceTrees,
    PassingTests,
    EvidenceEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundArtifact {
    pub kind: ArtifactKind,
    pub name: Alias,
    pub content: ContentId,
    pub binding: ContentId,
}

/// The exact request a checker producer port receives for one attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckerRequest {
    pub request: AdmittedRequest,
    pub attempt: AttemptBinding,
}

/// Result evidence a checker producer returns for one attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckerObservation {
    pub attempt: ContentId,
    pub binding: RequestBinding,
    pub checker: CheckerIdentity,
    pub evidence_kind: EvidenceKind,
    pub verdict: CheckerVerdict,
    pub coverage: Vec<Alias>,
    pub output_complete: bool,
    pub receipt: Option<ContentId>,
    pub artifacts: Vec<BoundArtifact>,
}

/// Producer-port failures that the shell reports for the current attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProducerFailure {
    TimedOut,
    Cancelled,
    ToolError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdmittedResult {
    pub attempt: AttemptBinding,
    pub binding: RequestBinding,
    pub checker: CheckerIdentity,
    pub outcome: SemanticOutcome,
    pub coverage: Vec<Alias>,
    pub receipt: Option<ContentId>,
    pub artifacts: Vec<BoundArtifact>,
    pub diagnostic: Option<Diagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Pending,
    Completed(SemanticOutcome),
    Cancelled,
    Superseded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt: AttemptBinding,
    pub status: AttemptStatus,
}

/// A cached result keyed by the full request identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: ContentId,
    pub result: AdmittedResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticCheckState {
    request: AdmittedRequest,
    attempts: Vec<AttemptRecord>,
    result: Option<AdmittedResult>,
    stale: Option<BindingComponent>,
    cancelled: bool,
}

impl SemanticCheckState {
    #[must_use]
    pub const fn new(request: AdmittedRequest) -> Self {
        Self {
            request,
            attempts: Vec::new(),
            result: None,
            stale: None,
            cancelled: false,
        }
    }

    #[must_use]
    pub const fn request(&self) -> &AdmittedRequest {
        &self.request
    }

    #[must_use]
    pub fn attempts(&self) -> &[AttemptRecord] {
        &self.attempts
    }

    #[must_use]
    pub const fn result(&self) -> Option<&AdmittedResult> {
        self.result.as_ref()
    }

    #[must_use]
    pub const fn stale(&self) -> Option<BindingComponent> {
        self.stale
    }

    #[must_use]
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }

    fn pending(&self) -> Option<&AttemptRecord> {
        self.attempts
            .iter()
            .find(|record| record.status == AttemptStatus::Pending)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticCheckCommand {
    Dispatch,
    ObserveResult(Box<CheckerObservation>),
    ObserveFailure {
        attempt: ContentId,
        failure: ProducerFailure,
    },
    Retry,
    Cancel,
    /// A fresh read-only re-observation of every bound input.
    ObserveInputs(Box<RequestBinding>),
    UseCache(Box<CacheEntry>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SemanticCheckEvent {
    AttemptStarted(AttemptBinding),
    ResultRecorded(Box<AdmittedResult>),
    CacheReused(Box<AdmittedResult>),
    EvaluationCancelled {
        attempt: Option<ContentId>,
    },
    InputsChanged {
        component: BindingComponent,
        superseded: Option<ContentId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum SemanticCheckEffect {
    DispatchChecker(Box<CheckerRequest>),
    CancelChecker(ContentId),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SemanticCheckDecision {
    pub events: Vec<SemanticCheckEvent>,
    pub effects: Vec<SemanticCheckEffect>,
}

fn error(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

fn start_attempt(state: &SemanticCheckState) -> CoreResult<SemanticCheckDecision> {
    let ordinal = u32::try_from(state.attempts.len())
        .ok()
        .and_then(|used| used.checked_add(1))
        .ok_or_else(|| {
            error(
                "semantic_attempt_bound",
                "semantic_merge.attempts",
                "attempt count exceeds its named bound",
            )
        })?;
    if ordinal > state.request.request().policy.retry.max_attempts {
        return Err(error(
            "semantic_attempt_bound",
            "semantic_merge.attempts",
            "attempt count exceeds the policy bound",
        ));
    }
    let attempt = attempt_binding(&state.request.binding().identity, ordinal)?;
    Ok(SemanticCheckDecision {
        events: alloc::vec![SemanticCheckEvent::AttemptStarted(attempt.clone())],
        effects: alloc::vec![SemanticCheckEffect::DispatchChecker(Box::new(
            CheckerRequest {
                request: state.request.clone(),
                attempt,
            }
        ))],
    })
}

/// Decides one command against the current state: `state + command -> events + effects`.
// r[impl kiln.semantic_merge.freshness]
// r[impl kiln.semantic_merge.binding]
pub fn decide(
    state: &SemanticCheckState,
    command: &SemanticCheckCommand,
) -> CoreResult<SemanticCheckDecision> {
    let empty = SemanticCheckDecision {
        events: Vec::new(),
        effects: Vec::new(),
    };
    if let Some(component) = state.stale {
        return match command {
            SemanticCheckCommand::ObserveInputs(_) => Ok(empty),
            SemanticCheckCommand::ObserveResult(_)
            | SemanticCheckCommand::ObserveFailure { .. } => Err(error(
                "semantic_result_stale",
                &stale_field(component),
                "evaluation is stale; results for the old inputs are rejected",
            )),
            SemanticCheckCommand::Dispatch
            | SemanticCheckCommand::Retry
            | SemanticCheckCommand::Cancel
            | SemanticCheckCommand::UseCache(_) => Err(error(
                "semantic_evaluation_stale",
                &stale_field(component),
                "inputs changed; a new request and evaluation are required",
            )),
        };
    }
    if state.cancelled {
        return match command {
            SemanticCheckCommand::ObserveResult(_)
            | SemanticCheckCommand::ObserveFailure { .. } => Err(error(
                "semantic_result_late",
                "semantic_merge.attempt",
                "result arrived after the evaluation was cancelled",
            )),
            SemanticCheckCommand::ObserveInputs(_) => Ok(empty),
            SemanticCheckCommand::Dispatch
            | SemanticCheckCommand::Retry
            | SemanticCheckCommand::Cancel
            | SemanticCheckCommand::UseCache(_) => Err(error(
                "semantic_evaluation_cancelled",
                "semantic_merge.evaluation",
                "evaluation was cancelled",
            )),
        };
    }
    match command {
        SemanticCheckCommand::Dispatch => {
            if !state.attempts.is_empty() || state.result.is_some() {
                return Err(error(
                    "semantic_dispatch_duplicate",
                    "semantic_merge.attempts",
                    "evaluation already dispatched; use retry for a new attempt",
                ));
            }
            start_attempt(state)
        }
        SemanticCheckCommand::ObserveResult(observation) => {
            let result = admit_observation(state, observation)?;
            Ok(SemanticCheckDecision {
                events: alloc::vec![SemanticCheckEvent::ResultRecorded(Box::new(result))],
                effects: Vec::new(),
            })
        }
        SemanticCheckCommand::ObserveFailure { attempt, failure } => {
            let record = current_attempt(state, attempt)?;
            let (outcome, code) = match failure {
                ProducerFailure::TimedOut => (
                    SemanticOutcome::Exhausted(ExhaustionBound::WallTime),
                    "semantic_producer_timed_out",
                ),
                ProducerFailure::Cancelled => {
                    (SemanticOutcome::Cancelled, "semantic_producer_cancelled")
                }
                ProducerFailure::ToolError => {
                    (SemanticOutcome::ToolError, "semantic_producer_failed")
                }
            };
            let result = AdmittedResult {
                attempt: record.attempt.clone(),
                binding: state.request.binding().clone(),
                checker: state.request.request().checker.clone(),
                outcome,
                coverage: Vec::new(),
                receipt: None,
                artifacts: Vec::new(),
                diagnostic: Some(error(
                    code,
                    "semantic_merge.producer",
                    "checker producer reported no admissible result",
                )),
            };
            Ok(SemanticCheckDecision {
                events: alloc::vec![SemanticCheckEvent::ResultRecorded(Box::new(result))],
                effects: Vec::new(),
            })
        }
        SemanticCheckCommand::Retry => {
            let Some(result) = &state.result else {
                return Err(error(
                    "semantic_retry_refused",
                    "semantic_merge.attempts",
                    "retry needs a completed attempt",
                ));
            };
            match classify_retry(
                &state.request.request().policy.retry,
                result.outcome,
                state.attempts.len(),
            ) {
                RetryDecision::Retry => start_attempt(state),
                RetryDecision::Terminal => Err(error(
                    "semantic_retry_refused",
                    "semantic_merge.policy.retry",
                    "outcome is terminal or the policy attempt bound is reached",
                )),
            }
        }
        SemanticCheckCommand::Cancel => {
            if state.result.is_some() && state.pending().is_none() {
                return Err(error(
                    "semantic_evaluation_terminal",
                    "semantic_merge.evaluation",
                    "evaluation already has a recorded outcome",
                ));
            }
            let attempt = state
                .pending()
                .map(|record| record.attempt.identity.clone());
            Ok(SemanticCheckDecision {
                effects: attempt
                    .iter()
                    .cloned()
                    .map(SemanticCheckEffect::CancelChecker)
                    .collect(),
                events: alloc::vec![SemanticCheckEvent::EvaluationCancelled { attempt }],
            })
        }
        SemanticCheckCommand::ObserveInputs(current) => {
            let Some(component) = state.request.binding().first_mismatch(current) else {
                return Ok(empty);
            };
            let superseded = state
                .pending()
                .map(|record| record.attempt.identity.clone());
            Ok(SemanticCheckDecision {
                effects: superseded
                    .iter()
                    .cloned()
                    .map(SemanticCheckEffect::CancelChecker)
                    .collect(),
                events: alloc::vec![SemanticCheckEvent::InputsChanged {
                    component,
                    superseded,
                }],
            })
        }
        SemanticCheckCommand::UseCache(entry) => {
            if !state.attempts.is_empty() || state.result.is_some() {
                return Err(error(
                    "semantic_dispatch_duplicate",
                    "semantic_merge.attempts",
                    "cache reuse is only admitted before the first attempt",
                ));
            }
            let result = admit_cache_entry(&state.request, entry)?;
            Ok(SemanticCheckDecision {
                events: alloc::vec![SemanticCheckEvent::CacheReused(Box::new(result))],
                effects: Vec::new(),
            })
        }
    }
}

fn stale_field(component: BindingComponent) -> alloc::string::String {
    alloc::format!("semantic_merge.binding.{}", component.name())
}

fn current_attempt<'a>(
    state: &'a SemanticCheckState,
    attempt: &ContentId,
) -> CoreResult<&'a AttemptRecord> {
    let Some(record) = state
        .attempts
        .iter()
        .find(|record| &record.attempt.identity == attempt)
    else {
        return Err(error(
            "semantic_attempt_replay",
            "semantic_merge.attempt",
            "result names an attempt this evaluation never issued",
        ));
    };
    if record.status != AttemptStatus::Pending {
        return Err(error(
            "semantic_result_late",
            "semantic_merge.attempt",
            "result belongs to a cancelled, superseded or completed attempt",
        ));
    }
    Ok(record)
}

// r[impl kiln.semantic_merge.binding]
// r[impl kiln.semantic_merge.independence]
fn admit_observation(
    state: &SemanticCheckState,
    observation: &CheckerObservation,
) -> CoreResult<AdmittedResult> {
    let record = current_attempt(state, &observation.attempt)?;
    let request = &state.request;
    require_binding(
        request.binding(),
        &observation.binding,
        "semantic_binding_mismatch",
    )?;
    if observation.checker != request.request().checker {
        return Err(error(
            "semantic_checker_substitution",
            "semantic_merge.binding.checker",
            "result was produced by a checker other than the admitted one",
        ));
    }
    if observation.evidence_kind != EvidenceKind::SemanticCheckerReceipt {
        return Err(error(
            "semantic_evidence_substitution",
            "semantic_merge.result.evidence_kind",
            "only a semantic checker receipt can carry a semantic merge result",
        ));
    }
    admit_artifacts(request, &record.attempt, &observation.artifacts)?;
    let required = &request.request().subject.required_coverage;
    let covered: BTreeSet<&Alias> = observation.coverage.iter().collect();
    if !strictly_sorted(&observation.coverage)
        || covered.iter().any(|unit| !required.contains(unit))
    {
        return Err(error(
            "semantic_coverage_scope",
            "semantic_merge.result.coverage",
            "result coverage is unsorted or names units outside the requested subject",
        ));
    }
    let mut diagnostic = None;
    let outcome = if observation.output_complete {
        SemanticOutcome::from_verdict(observation.verdict)
    } else {
        diagnostic = Some(error(
            "semantic_partial_output",
            "semantic_merge.result.output_complete",
            "checker output is incomplete; the attempt is recorded as a tool error",
        ));
        SemanticOutcome::ToolError
    };
    if matches!(
        outcome,
        SemanticOutcome::Verified | SemanticOutcome::Counterexample
    ) && observation.receipt.is_none()
    {
        return Err(error(
            "semantic_receipt_missing",
            "semantic_merge.result.receipt",
            "verified and counterexample results require a checker receipt",
        ));
    }
    if outcome == SemanticOutcome::Verified && covered.len() != required.len() {
        return Err(error(
            "semantic_coverage_missing",
            "semantic_merge.result.coverage",
            "verified result does not cover every required unit",
        ));
    }
    Ok(AdmittedResult {
        attempt: record.attempt.clone(),
        binding: observation.binding.clone(),
        checker: observation.checker.clone(),
        outcome,
        coverage: observation.coverage.clone(),
        receipt: observation.receipt.clone(),
        artifacts: observation.artifacts.clone(),
        diagnostic,
    })
}

fn admit_artifacts(
    request: &AdmittedRequest,
    attempt: &AttemptBinding,
    artifacts: &[BoundArtifact],
) -> CoreResult<()> {
    let products = artifacts
        .iter()
        .filter(|artifact| artifact.kind == ArtifactKind::Product)
        .count();
    if products > request.request().bounds.max_products {
        return Err(error(
            "semantic_product_bound",
            "semantic_merge.result.artifacts",
            "result product count exceeds its named bound",
        ));
    }
    let mut names = BTreeSet::new();
    for artifact in artifacts {
        if !names.insert((artifact.kind, &artifact.name)) {
            return Err(error(
                "semantic_artifact_duplicate",
                "semantic_merge.result.artifacts",
                "result names one artifact twice",
            ));
        }
        let expected = artifact_binding(
            &request.binding().identity,
            &attempt.identity,
            artifact.kind,
            artifact.name.as_str(),
            &artifact.content,
        )?;
        if expected != artifact.binding {
            return Err(error(
                "semantic_artifact_binding",
                "semantic_merge.result.artifacts",
                "log or product is bound to a different request or attempt",
            ));
        }
    }
    Ok(())
}

/// Applies one event: `state + event -> state`.
pub fn evolve(
    state: &SemanticCheckState,
    event: &SemanticCheckEvent,
) -> CoreResult<SemanticCheckState> {
    let mut next = state.clone();
    match event {
        SemanticCheckEvent::AttemptStarted(attempt) => {
            if attempt.request != next.request.binding().identity || next.pending().is_some() {
                return Err(error(
                    "semantic_event_order",
                    "semantic_merge.attempts",
                    "attempt start does not follow the recorded state",
                ));
            }
            next.result = None;
            next.attempts.push(AttemptRecord {
                attempt: attempt.clone(),
                status: AttemptStatus::Pending,
            });
        }
        SemanticCheckEvent::ResultRecorded(result) => {
            let Some(record) = next.attempts.iter_mut().find(|record| {
                record.attempt == result.attempt && record.status == AttemptStatus::Pending
            }) else {
                return Err(error(
                    "semantic_event_order",
                    "semantic_merge.attempts",
                    "result does not match a pending attempt",
                ));
            };
            record.status = AttemptStatus::Completed(result.outcome);
            next.result = Some(result.as_ref().clone());
        }
        SemanticCheckEvent::CacheReused(result) => {
            if !next.attempts.is_empty() {
                return Err(error(
                    "semantic_event_order",
                    "semantic_merge.attempts",
                    "cache reuse must precede every attempt",
                ));
            }
            next.result = Some(result.as_ref().clone());
        }
        SemanticCheckEvent::EvaluationCancelled { attempt } => {
            mark(&mut next, attempt.as_ref(), AttemptStatus::Cancelled)?;
            next.cancelled = true;
        }
        SemanticCheckEvent::InputsChanged {
            component,
            superseded,
        } => {
            mark(&mut next, superseded.as_ref(), AttemptStatus::Superseded)?;
            next.stale = Some(*component);
        }
    }
    Ok(next)
}

fn mark(
    state: &mut SemanticCheckState,
    attempt: Option<&ContentId>,
    status: AttemptStatus,
) -> CoreResult<()> {
    let Some(attempt) = attempt else {
        return Ok(());
    };
    let Some(record) = state.attempts.iter_mut().find(|record| {
        &record.attempt.identity == attempt && record.status == AttemptStatus::Pending
    }) else {
        return Err(error(
            "semantic_event_order",
            "semantic_merge.attempts",
            "event names an attempt that is not pending",
        ));
    };
    record.status = status;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDecision {
    Retry,
    Terminal,
}

/// Pure retry classification: only tool errors (and, if allowed, exhaustion) retry, within bound.
// r[impl kiln.semantic_merge.binding]
#[must_use]
pub const fn classify_retry(
    policy: &RetryPolicy,
    outcome: SemanticOutcome,
    attempts_used: usize,
) -> RetryDecision {
    let eligible = match outcome {
        SemanticOutcome::ToolError => policy.retry_tool_error,
        SemanticOutcome::Exhausted(_) => policy.retry_exhausted,
        SemanticOutcome::Verified
        | SemanticOutcome::Counterexample
        | SemanticOutcome::Unknown
        | SemanticOutcome::Unsupported
        | SemanticOutcome::Cancelled
        | SemanticOutcome::Stale(_) => false,
    };
    if eligible && attempts_used < policy.max_attempts as usize {
        RetryDecision::Retry
    } else {
        RetryDecision::Terminal
    }
}

/// A cache entry for the recorded result, only for deterministic outcomes.
#[must_use]
pub fn cache_entry(state: &SemanticCheckState) -> Option<CacheEntry> {
    let result = state.result.as_ref()?;
    if state.stale.is_some() || state.cancelled || !result.outcome.is_deterministic() {
        return None;
    }
    Some(CacheEntry {
        key: state.request.cache_key().clone(),
        result: result.clone(),
    })
}

/// Admits a cached result only for the identical request, checker and deterministic outcome.
// r[impl kiln.semantic_merge.binding]
pub fn admit_cache_entry(
    request: &AdmittedRequest,
    entry: &CacheEntry,
) -> CoreResult<AdmittedResult> {
    require_binding(
        request.binding(),
        &entry.result.binding,
        "semantic_cache_mismatch",
    )?;
    if &entry.key != request.cache_key() || entry.result.attempt.request != entry.key {
        return Err(error(
            "semantic_cache_mismatch",
            "semantic_merge.cache.key",
            "cache key differs from the request identity",
        ));
    }
    if entry.result.checker != request.request().checker {
        return Err(error(
            "semantic_cache_mismatch",
            "semantic_merge.binding.checker",
            "cached result was produced by another checker",
        ));
    }
    if !entry.result.outcome.is_deterministic() {
        return Err(error(
            "semantic_cache_nondeterministic",
            "semantic_merge.cache.outcome",
            "only verified, counterexample and unsupported results may be reused",
        ));
    }
    Ok(entry.result.clone())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticGateVerdict {
    pub request: ContentId,
    pub mode: CheckMode,
    /// `None` while no outcome is recorded.
    pub outcome: Option<SemanticOutcome>,
    pub satisfies_required: bool,
    pub blocks: bool,
    pub conflict_demonstrated: bool,
    pub diagnostic: Option<Diagnostic>,
    pub revalidation: RevalidationObligation,
}

/// Decides the semantic gate against a fresh re-observation of every bound input.
// r[impl kiln.semantic_merge.outcomes]
// r[impl kiln.semantic_merge.freshness]
pub fn decide_gate(
    state: &SemanticCheckState,
    fresh: &RequestBinding,
) -> CoreResult<SemanticGateVerdict> {
    let request = &state.request;
    let mut diagnostic = None;
    let stale = state
        .stale
        .or_else(|| request.binding().first_mismatch(fresh));
    let outcome = if let Some(component) = stale {
        diagnostic = Some(error(
            "semantic_result_stale",
            &stale_field(component),
            "an input changed after the check; the result no longer applies",
        ));
        Some(SemanticOutcome::Stale(component))
    } else if state.cancelled {
        Some(SemanticOutcome::Cancelled)
    } else {
        state.result.as_ref().map(|result| {
            diagnostic.clone_from(&result.diagnostic);
            result.outcome
        })
    };
    let applicable = state.result.as_ref().is_some_and(|result| {
        result.binding == *request.binding()
            && result.checker == request.request().checker
            && request
                .request()
                .policy
                .admitted_checkers
                .contains(&result.checker)
            && result.coverage == request.request().subject.required_coverage
    });
    let mode = request.request().policy.mode;
    let satisfies_required =
        mode == CheckMode::Required && applicable && outcome == Some(SemanticOutcome::Verified);
    Ok(SemanticGateVerdict {
        request: request.binding().identity.clone(),
        mode,
        outcome,
        satisfies_required,
        blocks: mode == CheckMode::Required && !satisfies_required,
        conflict_demonstrated: outcome.is_some_and(SemanticOutcome::conflict_demonstrated),
        diagnostic,
        revalidation: RevalidationObligation::for_request(request),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndependentGateKind {
    Build,
    Test,
    Lint,
    Review,
    Other,
}

/// An independently requested gate result, already bound to the same candidate set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndependentGate {
    pub name: Alias,
    pub kind: IndependentGateKind,
    pub required: bool,
    pub passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CombinedGateDecision {
    pub passed: bool,
    pub semantic_blocking: bool,
    pub failing_gates: Vec<Alias>,
}

/// Combines the semantic gate with independent gates; neither substitutes for the other.
// r[impl kiln.semantic_merge.independence]
pub fn combine_gates(
    semantic: &SemanticGateVerdict,
    others: &[IndependentGate],
) -> CoreResult<CombinedGateDecision> {
    if others.len() > MAX_INDEPENDENT_GATES {
        return Err(error(
            "semantic_gate_bound",
            "semantic_merge.gates",
            "independent gate count exceeds its named bound",
        ));
    }
    let mut names = BTreeSet::new();
    let mut failing_gates = Vec::new();
    for gate in others {
        if !names.insert(&gate.name) {
            return Err(error(
                "semantic_gate_duplicate",
                "semantic_merge.gates",
                "independent gate name is duplicated",
            ));
        }
        if gate.required && !gate.passed {
            failing_gates.push(gate.name.clone());
        }
    }
    let semantic_blocking = semantic.mode == CheckMode::Required && !semantic.satisfies_required;
    Ok(CombinedGateDecision {
        passed: failing_gates.is_empty() && !semantic_blocking,
        semantic_blocking,
        failing_gates,
    })
}
