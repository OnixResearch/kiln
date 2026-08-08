use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CommandId, ContentId, CoreResult, Diagnostic, EffectId,
    EvaluationId, ExternalRef, Limits, PipelineRevisionId, ProjectId, ProviderProfileId, RunId,
    TaskId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub alias: Alias,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PipelineRevision {
    pub id: PipelineRevisionId,
    pub project_id: ProjectId,
    pub canonical_identity: ContentId,
    pub tasks: Vec<TaskDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: TaskId,
    pub build_provider: ProviderProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub native: ExternalRef,
    pub source_snapshot: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub id: EvaluationId,
    pub project_id: ProjectId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub source_snapshot: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Outcome {
    Pending,
    Success,
    DomainFailure,
    Denied,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

impl Outcome {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Success
                | Self::DomainFailure
                | Self::Denied
                | Self::Cancelled
                | Self::TimedOut
                | Self::ProviderError
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Product {
    pub identity: ContentId,
    pub producing_attempt: AttemptId,
    pub provider_reference: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub name: Alias,
    pub value: i64,
    pub producing_attempt: AttemptId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationIntent {
    pub run_id: RunId,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptState {
    pub run_id: RunId,
    pub provider_profile: ProviderProfileId,
    pub outcome: Outcome,
    pub external_reference: Option<ExternalRef>,
    pub products: Vec<Product>,
    pub metrics: Vec<Metric>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    pub active_attempt: AttemptId,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub revision: u64,
    pub runs: BTreeMap<RunId, RunState>,
    pub attempts: BTreeMap<AttemptId, AttemptState>,
    pub holds: BTreeSet<ExternalRef>,
    pub seen_commands: BTreeSet<CommandId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub contract_version: u16,
    pub command_id: CommandId,
    pub expected_revision: u64,
    pub command: Command,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Command {
    StartRun {
        run_id: RunId,
        attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
        effect_id: EffectId,
    },
    CompleteAttempt {
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
        products: Vec<Product>,
        metrics: Vec<Metric>,
    },
    RecordUnknown {
        attempt_id: AttemptId,
        external_reference: Option<ExternalRef>,
    },
    ReconcileAttempt {
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
    },
    Retry {
        run_id: RunId,
        prior_attempt_id: AttemptId,
        next_attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
        effect_id: EffectId,
    },
    Cancel {
        attempt_id: AttemptId,
        effect_id: EffectId,
    },
    SetHold {
        scope: ExternalRef,
        held: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Event {
    RunStarted {
        command_id: CommandId,
        run_id: RunId,
        attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
    },
    AttemptCompleted {
        command_id: CommandId,
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
        products: Vec<Product>,
        metrics: Vec<Metric>,
    },
    AttemptUnknown {
        command_id: CommandId,
        attempt_id: AttemptId,
        external_reference: Option<ExternalRef>,
    },
    AttemptReconciled {
        command_id: CommandId,
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
    },
    RetryStarted {
        command_id: CommandId,
        run_id: RunId,
        prior_attempt_id: AttemptId,
        next_attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
    },
    CancellationRequested {
        command_id: CommandId,
        attempt_id: AttemptId,
    },
    HoldChanged {
        command_id: CommandId,
        scope: ExternalRef,
        held: bool,
    },
}

impl Event {
    #[must_use]
    pub fn command_id(&self) -> &CommandId {
        match self {
            Self::RunStarted { command_id, .. }
            | Self::AttemptCompleted { command_id, .. }
            | Self::AttemptUnknown { command_id, .. }
            | Self::AttemptReconciled { command_id, .. }
            | Self::RetryStarted { command_id, .. }
            | Self::CancellationRequested { command_id, .. }
            | Self::HoldChanged { command_id, .. } => command_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Dispatch {
        effect_id: EffectId,
        attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
        idempotency_key: CommandId,
    },
    Cancel {
        effect_id: EffectId,
        attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
        idempotency_key: CommandId,
    },
    Reconcile {
        effect_id: EffectId,
        attempt_id: AttemptId,
        provider_profile: ProviderProfileId,
        idempotency_key: CommandId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub events: Vec<Event>,
    pub effects: Vec<Effect>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn decide(state: &State, envelope: &CommandEnvelope, limits: Limits) -> CoreResult<Decision> {
    if envelope.contract_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "unsupported_version",
            "command.contract_version",
            "command version is unsupported",
        ));
    }
    if envelope.expected_revision != state.revision {
        return Err(Diagnostic::new(
            "stale_command",
            "command.expected_revision",
            "command expected revision is stale",
        ));
    }
    if state.seen_commands.contains(&envelope.command_id) {
        return Err(Diagnostic::new(
            "duplicate_command",
            "command.command_id",
            "command identity was already applied",
        ));
    }

    let decision = decide_command(state, envelope)?;
    if decision.events.len() > limits.max_transition_items
        || decision.effects.len() > limits.max_transition_items
    {
        return Err(Diagnostic::new(
            "transition_bound_exceeded",
            "decision",
            "decision exceeds the named transition bound",
        ));
    }
    Ok(decision)
}

fn decide_command(state: &State, envelope: &CommandEnvelope) -> CoreResult<Decision> {
    match &envelope.command {
        Command::StartRun {
            run_id,
            attempt_id,
            provider_profile,
            effect_id,
        } => {
            if state.runs.contains_key(run_id) || state.attempts.contains_key(attempt_id) {
                return Err(Diagnostic::new(
                    "identity_reused",
                    "start_run",
                    "run or attempt identity is already in use",
                ));
            }
            Ok(Decision {
                events: vec![Event::RunStarted {
                    command_id: envelope.command_id.clone(),
                    run_id: run_id.clone(),
                    attempt_id: attempt_id.clone(),
                    provider_profile: provider_profile.clone(),
                }],
                effects: vec![Effect::Dispatch {
                    effect_id: effect_id.clone(),
                    attempt_id: attempt_id.clone(),
                    provider_profile: provider_profile.clone(),
                    idempotency_key: envelope.command_id.clone(),
                }],
                diagnostics: Vec::new(),
            })
        }
        Command::CompleteAttempt {
            attempt_id,
            outcome,
            external_reference,
            products,
            metrics,
        } => {
            let _attempt = active_attempt(state, attempt_id)?;
            if !outcome.is_terminal() {
                return Err(Diagnostic::new(
                    "non_terminal_completion",
                    "complete_attempt.outcome",
                    "normal completion requires a terminal outcome",
                ));
            }
            validate_products_and_metrics(attempt_id, products, metrics)?;
            if let Some(reference) = external_reference {
                reference.validate()?;
            }
            Ok(Decision {
                events: vec![Event::AttemptCompleted {
                    command_id: envelope.command_id.clone(),
                    attempt_id: attempt_id.clone(),
                    outcome: *outcome,
                    external_reference: external_reference.clone(),
                    products: products.clone(),
                    metrics: metrics.clone(),
                }],
                effects: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
        Command::RecordUnknown {
            attempt_id,
            external_reference,
        } => {
            let _attempt = active_attempt(state, attempt_id)?;
            Ok(Decision {
                events: vec![Event::AttemptUnknown {
                    command_id: envelope.command_id.clone(),
                    attempt_id: attempt_id.clone(),
                    external_reference: external_reference.clone(),
                }],
                effects: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
        Command::ReconcileAttempt {
            attempt_id,
            outcome,
            external_reference,
        } => {
            let attempt = state.attempts.get(attempt_id).ok_or(Diagnostic::new(
                "missing_attempt",
                "reconcile_attempt.attempt_id",
                "attempt does not exist",
            ))?;
            if attempt.outcome != Outcome::Unknown {
                return Err(Diagnostic::new(
                    "not_unknown",
                    "reconcile_attempt.attempt_id",
                    "only an unknown attempt can be reconciled",
                ));
            }
            if *outcome == Outcome::Pending || *outcome == Outcome::Unknown {
                return Err(Diagnostic::new(
                    "invalid_reconciliation",
                    "reconcile_attempt.outcome",
                    "reconciliation must resolve the unknown outcome",
                ));
            }
            Ok(Decision {
                events: vec![Event::AttemptReconciled {
                    command_id: envelope.command_id.clone(),
                    attempt_id: attempt_id.clone(),
                    outcome: *outcome,
                    external_reference: external_reference.clone(),
                }],
                effects: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
        Command::Retry {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
            effect_id,
        } => {
            let prior = state.attempts.get(prior_attempt_id).ok_or(Diagnostic::new(
                "missing_attempt",
                "retry.prior_attempt_id",
                "prior attempt does not exist",
            ))?;
            if prior.run_id != *run_id {
                return Err(Diagnostic::new(
                    "attempt_run_mismatch",
                    "retry.run_id",
                    "prior attempt belongs to another run",
                ));
            }
            if prior.outcome == Outcome::Pending {
                return Err(Diagnostic::new(
                    "attempt_active",
                    "retry.prior_attempt_id",
                    "an active attempt cannot be retried",
                ));
            }
            if state.attempts.contains_key(next_attempt_id) || prior_attempt_id == next_attempt_id {
                return Err(Diagnostic::new(
                    "attempt_identity_reused",
                    "retry.next_attempt_id",
                    "retry must use a distinct new attempt identity",
                ));
            }
            Ok(Decision {
                events: vec![Event::RetryStarted {
                    command_id: envelope.command_id.clone(),
                    run_id: run_id.clone(),
                    prior_attempt_id: prior_attempt_id.clone(),
                    next_attempt_id: next_attempt_id.clone(),
                    provider_profile: provider_profile.clone(),
                }],
                effects: vec![Effect::Dispatch {
                    effect_id: effect_id.clone(),
                    attempt_id: next_attempt_id.clone(),
                    provider_profile: provider_profile.clone(),
                    idempotency_key: envelope.command_id.clone(),
                }],
                diagnostics: Vec::new(),
            })
        }
        Command::Cancel {
            attempt_id,
            effect_id,
        } => {
            let attempt = active_attempt(state, attempt_id)?;
            Ok(Decision {
                events: vec![Event::CancellationRequested {
                    command_id: envelope.command_id.clone(),
                    attempt_id: attempt_id.clone(),
                }],
                effects: vec![Effect::Cancel {
                    effect_id: effect_id.clone(),
                    attempt_id: attempt_id.clone(),
                    provider_profile: attempt.provider_profile.clone(),
                    idempotency_key: envelope.command_id.clone(),
                }],
                diagnostics: Vec::new(),
            })
        }
        Command::SetHold { scope, held } => {
            scope.validate()?;
            if *held == state.holds.contains(scope) {
                return Err(Diagnostic::new(
                    "hold_state_unchanged",
                    "set_hold.scope",
                    "hold command does not change the admitted hold state",
                ));
            }
            Ok(Decision {
                events: vec![Event::HoldChanged {
                    command_id: envelope.command_id.clone(),
                    scope: scope.clone(),
                    held: *held,
                }],
                effects: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
    }
}

fn active_attempt<'a>(state: &'a State, attempt_id: &AttemptId) -> CoreResult<&'a AttemptState> {
    let attempt = state.attempts.get(attempt_id).ok_or(Diagnostic::new(
        "missing_attempt",
        "attempt_id",
        "attempt does not exist",
    ))?;
    if attempt.outcome != Outcome::Pending {
        return Err(Diagnostic::new(
            "terminal_mutation",
            "attempt_id",
            "attempt is not active",
        ));
    }
    Ok(attempt)
}

fn validate_products_and_metrics(
    attempt_id: &AttemptId,
    products: &[Product],
    metrics: &[Metric],
) -> CoreResult<()> {
    if products
        .iter()
        .any(|product| product.producing_attempt != *attempt_id)
    {
        return Err(Diagnostic::new(
            "orphan_product",
            "products.producing_attempt",
            "product belongs to another attempt",
        ));
    }
    if metrics
        .iter()
        .any(|metric| metric.producing_attempt != *attempt_id)
    {
        return Err(Diagnostic::new(
            "orphan_metric",
            "metrics.producing_attempt",
            "metric belongs to another attempt",
        ));
    }
    Ok(())
}

pub fn evolve(state: &State, event: &Event) -> CoreResult<State> {
    if state.seen_commands.contains(event.command_id()) {
        return Err(Diagnostic::new(
            "duplicate_event",
            "event.command_id",
            "event command identity was already applied",
        ));
    }
    let mut next = state.clone();
    match event {
        Event::RunStarted {
            run_id,
            attempt_id,
            provider_profile,
            ..
        } => {
            if next.runs.contains_key(run_id) || next.attempts.contains_key(attempt_id) {
                return Err(Diagnostic::new(
                    "invalid_history",
                    "run_started",
                    "history reuses a run or attempt identity",
                ));
            }
            next.runs.insert(
                run_id.clone(),
                RunState {
                    active_attempt: attempt_id.clone(),
                    outcome: Outcome::Pending,
                },
            );
            next.attempts.insert(
                attempt_id.clone(),
                AttemptState {
                    run_id: run_id.clone(),
                    provider_profile: provider_profile.clone(),
                    outcome: Outcome::Pending,
                    external_reference: None,
                    products: Vec::new(),
                    metrics: Vec::new(),
                },
            );
        }
        Event::AttemptCompleted {
            attempt_id,
            outcome,
            external_reference,
            ..
        }
        | Event::AttemptReconciled {
            attempt_id,
            outcome,
            external_reference,
            ..
        } => {
            let attempt = next.attempts.get_mut(attempt_id).ok_or(Diagnostic::new(
                "invalid_history",
                "attempt_completed",
                "history names a missing attempt",
            ))?;
            if attempt.outcome.is_terminal() {
                return Err(Diagnostic::new(
                    "invalid_history",
                    "attempt_completed",
                    "history mutates a terminal attempt",
                ));
            }
            attempt.outcome = *outcome;
            attempt.external_reference = external_reference.clone();
            if let Event::AttemptCompleted {
                products, metrics, ..
            } = event
            {
                attempt.products = products.clone();
                attempt.metrics = metrics.clone();
            }
            let run = next.runs.get_mut(&attempt.run_id).ok_or(Diagnostic::new(
                "invalid_history",
                "attempt_completed",
                "attempt run is missing",
            ))?;
            run.outcome = *outcome;
        }
        Event::AttemptUnknown {
            attempt_id,
            external_reference,
            ..
        } => {
            let attempt = next.attempts.get_mut(attempt_id).ok_or(Diagnostic::new(
                "invalid_history",
                "attempt_unknown",
                "history names a missing attempt",
            ))?;
            if attempt.outcome != Outcome::Pending {
                return Err(Diagnostic::new(
                    "invalid_history",
                    "attempt_unknown",
                    "only a pending attempt can become unknown",
                ));
            }
            attempt.outcome = Outcome::Unknown;
            attempt.external_reference = external_reference.clone();
            let run = next.runs.get_mut(&attempt.run_id).ok_or(Diagnostic::new(
                "invalid_history",
                "attempt_unknown",
                "attempt run is missing",
            ))?;
            run.outcome = Outcome::Unknown;
        }
        Event::RetryStarted {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
            ..
        } => {
            let prior = next.attempts.get(prior_attempt_id).ok_or(Diagnostic::new(
                "invalid_history",
                "retry_started",
                "prior attempt is missing",
            ))?;
            if prior.run_id != *run_id
                || prior.outcome == Outcome::Pending
                || next.attempts.contains_key(next_attempt_id)
            {
                return Err(Diagnostic::new(
                    "invalid_history",
                    "retry_started",
                    "retry history violates attempt identity or lifecycle rules",
                ));
            }
            next.attempts.insert(
                next_attempt_id.clone(),
                AttemptState {
                    run_id: run_id.clone(),
                    provider_profile: provider_profile.clone(),
                    outcome: Outcome::Pending,
                    external_reference: None,
                    products: Vec::new(),
                    metrics: Vec::new(),
                },
            );
            let run = next.runs.get_mut(run_id).ok_or(Diagnostic::new(
                "invalid_history",
                "retry_started",
                "run is missing",
            ))?;
            run.active_attempt = next_attempt_id.clone();
            run.outcome = Outcome::Pending;
        }
        Event::CancellationRequested { attempt_id, .. } => {
            let _attempt = next.attempts.get(attempt_id).ok_or(Diagnostic::new(
                "invalid_history",
                "cancellation_requested",
                "attempt is missing",
            ))?;
        }
        Event::HoldChanged { scope, held, .. } => {
            if *held == next.holds.contains(scope) {
                return Err(Diagnostic::new(
                    "invalid_history",
                    "hold_changed",
                    "hold event does not change state",
                ));
            }
            if *held {
                next.holds.insert(scope.clone());
            } else {
                next.holds.remove(scope);
            }
        }
    }
    next.seen_commands.insert(event.command_id().clone());
    next.revision = next.revision.checked_add(1).ok_or(Diagnostic::new(
        "revision_overflow",
        "state.revision",
        "state revision overflowed",
    ))?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INITIAL_REVISION: u64 = 0;

    fn start_command() -> CoreResult<CommandEnvelope> {
        Ok(CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new("command-start")?,
            expected_revision: INITIAL_REVISION,
            command: Command::StartRun {
                run_id: RunId::new("run-1")?,
                attempt_id: AttemptId::new("attempt-1")?,
                provider_profile: ProviderProfileId::new("standalone-build-v1")?,
                effect_id: EffectId::new("effect-1")?,
            },
        })
    }

    #[test]
    fn starts_run_deterministically() -> CoreResult<()> {
        let state = State::default();
        let command = start_command()?;
        let first = decide(&state, &command, Limits::default());
        let second = decide(&state, &command, Limits::default());
        assert_eq!(first, second);
        assert!(first.is_ok());
        Ok(())
    }

    #[test]
    fn rejects_terminal_mutation() -> CoreResult<()> {
        let state = State::default();
        let start = decide(&state, &start_command()?, Limits::default())?;
        let start_event = start.events.first().ok_or(Diagnostic::new(
            "missing_fixture_event",
            "start.events",
            "start fixture emitted no event",
        ))?;
        let started = evolve(&state, start_event)?;
        let completion = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new("command-complete")?,
            expected_revision: started.revision,
            command: Command::CompleteAttempt {
                attempt_id: AttemptId::new("attempt-1")?,
                outcome: Outcome::DomainFailure,
                external_reference: None,
                products: Vec::new(),
                metrics: Vec::new(),
            },
        };
        let decided = decide(&started, &completion, Limits::default())?;
        let completion_event = decided.events.first().ok_or(Diagnostic::new(
            "missing_fixture_event",
            "completion.events",
            "completion fixture emitted no event",
        ))?;
        let terminal = evolve(&started, completion_event)?;
        let late = CommandEnvelope {
            expected_revision: terminal.revision,
            command_id: CommandId::new("late-success")?,
            command: Command::CompleteAttempt {
                attempt_id: AttemptId::new("attempt-1")?,
                outcome: Outcome::Success,
                external_reference: None,
                products: Vec::new(),
                metrics: Vec::new(),
            },
            contract_version: CONTRACT_VERSION_V1,
        };
        assert!(decide(&terminal, &late, Limits::default()).is_err());
        Ok(())
    }

    #[test]
    fn unknown_outcome_is_not_terminal_success() {
        assert!(!Outcome::Unknown.is_terminal());
        assert!(Outcome::Success.is_terminal());
    }
}
