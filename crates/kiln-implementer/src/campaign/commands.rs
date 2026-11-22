//! Typed operator intents applied with their durable command record in one
//! canonical campaign transaction. A request ID makes a retry return the
//! recorded outcome instead of transitioning twice.

use kiln_core::implementation::campaign::{
    CampaignContract, CampaignState, DecisionRequestState, MAX_ID_BYTES,
};
use kiln_core::types::ContentId;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::{CampaignStore, TransitionFailure, decode, issue_in, start_in, text, transition_in};
use kiln_core::implementation::continuation::WakeKind;

pub const OPERATOR_COMMAND_SCHEMA: &str = "kiln.campaign-operator-command.v1";
/// Retained command records. A full record refuses new commands rather than
/// evicting the history that replays depend on.
pub const MAX_OPERATOR_COMMANDS: i64 = 65_536;
pub const MIN_REQUEST_ID_BYTES: usize = 8;
pub const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_ACTOR_BYTES: usize = 256;
const MAX_ANSWER_BYTES: usize = 512;
const MAX_OUTCOME_BYTES: usize = 64 * 1024;

/// One operator intent. It names campaign state only by identity and expected
/// version; it carries no paths, executables, credentials or observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperatorIntent {
    Start {
        contract: CampaignContract,
    },
    Continue {
        campaign_id: String,
        expected_version: u64,
    },
    Pause {
        campaign_id: String,
        expected_version: u64,
    },
    Cancel {
        campaign_id: String,
        expected_version: u64,
    },
    IssueDecision {
        campaign_id: String,
        expected_version: u64,
        identity: ContentId,
    },
    ResolveDecision {
        campaign_id: String,
        expected_version: u64,
        identity: ContentId,
        work_identity: Option<ContentId>,
        policy_identity: ContentId,
        candidate_identity: Option<ContentId>,
        answer: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorCommandKind {
    Start,
    Continue,
    Pause,
    Cancel,
    IssueDecision,
    ResolveDecision,
}

impl OperatorCommandKind {
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Continue,
        Self::Pause,
        Self::Cancel,
        Self::IssueDecision,
        Self::ResolveDecision,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Continue => "continue",
            Self::Pause => "pause",
            Self::Cancel => "cancel",
            Self::IssueDecision => "issue_decision",
            Self::ResolveDecision => "resolve_decision",
        }
    }
}

impl OperatorIntent {
    #[must_use]
    pub const fn kind(&self) -> OperatorCommandKind {
        match self {
            Self::Start { .. } => OperatorCommandKind::Start,
            Self::Continue { .. } => OperatorCommandKind::Continue,
            Self::Pause { .. } => OperatorCommandKind::Pause,
            Self::Cancel { .. } => OperatorCommandKind::Cancel,
            Self::IssueDecision { .. } => OperatorCommandKind::IssueDecision,
            Self::ResolveDecision { .. } => OperatorCommandKind::ResolveDecision,
        }
    }

    #[must_use]
    pub fn campaign_id(&self) -> &str {
        match self {
            Self::Start { contract } => &contract.campaign_id,
            Self::Continue { campaign_id, .. }
            | Self::Pause { campaign_id, .. }
            | Self::Cancel { campaign_id, .. }
            | Self::IssueDecision { campaign_id, .. }
            | Self::ResolveDecision { campaign_id, .. } => campaign_id,
        }
    }
}

/// Successor admission as the owner records it. None of these states
/// observes worker termination, provider cleanup or refunded usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionState {
    Open,
    Paused,
    Cancelled,
}

impl AdmissionState {
    #[must_use]
    pub const fn of(state: &CampaignState) -> Self {
        if state.cancelled {
            Self::Cancelled
        } else if state.paused {
            Self::Paused
        } else {
            Self::Open
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOutcome {
    pub identity: ContentId,
    pub state: DecisionRequestState,
}

/// The recorded result of one applied command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorOutcome {
    pub request_id: String,
    pub actor: String,
    pub command: OperatorCommandKind,
    pub campaign_id: String,
    /// Campaign store version after the transition.
    pub store_version: u64,
    pub admission: AdmissionState,
    pub decision: Option<DecisionOutcome>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorCommand {
    pub request_id: String,
    /// The principal the receiving shell admitted.
    pub actor: String,
    pub intent: OperatorIntent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedCommand {
    pub outcome: OperatorOutcome,
    /// The outcome was recorded by an earlier submission of this request.
    pub replayed: bool,
}

/// Why a command did not apply. Nothing was transitioned or recorded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorFailure {
    Invalid(&'static str),
    Missing,
    Stale {
        expected: u64,
        current: u64,
    },
    /// The request ID was recorded for another intent or principal.
    RequestConflict,
    /// The owner refused the transition with a stable diagnostic code.
    Refused {
        code: String,
        message: String,
    },
    RetentionExhausted,
    Storage(String),
}

impl From<TransitionFailure> for OperatorFailure {
    fn from(failure: TransitionFailure) -> Self {
        match failure {
            TransitionFailure::Missing => Self::Missing,
            TransitionFailure::Stale { expected, current } => Self::Stale { expected, current },
            TransitionFailure::Owner(error) => Self::Refused {
                code: error.code,
                message: error.message,
            },
            TransitionFailure::Storage(error) => Self::Storage(error),
        }
    }
}

fn storage(error: impl std::fmt::Display) -> OperatorFailure {
    OperatorFailure::Storage(text(error))
}

/// Request IDs are opaque bounded tokens chosen by the client.
#[must_use]
pub fn valid_request_id(value: &str) -> bool {
    (MIN_REQUEST_ID_BYTES..=MAX_REQUEST_ID_BYTES).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn validate(command: &OperatorCommand) -> Result<(), OperatorFailure> {
    if !valid_request_id(&command.request_id) {
        return Err(OperatorFailure::Invalid("request_id"));
    }
    if command.actor.trim().is_empty()
        || command.actor.len() > MAX_ACTOR_BYTES
        || command.actor.chars().any(char::is_control)
    {
        return Err(OperatorFailure::Invalid("actor"));
    }
    let campaign_id = command.intent.campaign_id();
    if campaign_id.trim().is_empty()
        || campaign_id.len() > MAX_ID_BYTES
        || campaign_id.chars().any(char::is_control)
    {
        return Err(OperatorFailure::Invalid("campaign_id"));
    }
    if let OperatorIntent::ResolveDecision { answer, .. } = &command.intent
        && (answer.is_empty() || answer.len() > MAX_ANSWER_BYTES)
    {
        return Err(OperatorFailure::Invalid("answer"));
    }
    Ok(())
}

/// BLAKE3 over the schema, the admitted principal and the intent, so another
/// principal cannot replay or take over a request ID.
fn fingerprint(command: &OperatorCommand) -> Result<String, OperatorFailure> {
    let bytes = serde_json::to_vec(&(OPERATOR_COMMAND_SCHEMA, &command.actor, &command.intent))
        .map_err(storage)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn decision_outcome(state: &CampaignState, identity: &ContentId) -> Option<DecisionOutcome> {
    state
        .decisions
        .get(identity)
        .map(|request| DecisionOutcome {
            identity: identity.clone(),
            state: request.state.clone(),
        })
}

impl CampaignStore {
    // r[impl kiln.implementation.campaign.controls]
    // r[impl kiln.implementation.campaign.controls.scenario.replay]
    /// Apply one operator intent and record its outcome in the same immediate
    /// transaction. Resubmitting an applied request returns its outcome.
    pub fn apply_operator(
        &mut self,
        command: OperatorCommand,
    ) -> Result<AppliedCommand, OperatorFailure> {
        validate(&command)?;
        let fingerprint = fingerprint(&command)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let recorded: Option<(String, Vec<u8>)> = tx
            .query_row(
                "SELECT fingerprint,outcome FROM implementation_campaign_command WHERE request_id=?1",
                params![command.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        if let Some((recorded_fingerprint, outcome)) = recorded {
            if recorded_fingerprint != fingerprint {
                return Err(OperatorFailure::RequestConflict);
            }
            let outcome: OperatorOutcome = decode(&outcome).map_err(OperatorFailure::Storage)?;
            return Ok(AppliedCommand {
                outcome,
                replayed: true,
            });
        }
        let retained: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM implementation_campaign_command",
                [],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if retained >= MAX_OPERATOR_COMMANDS {
            return Err(OperatorFailure::RetentionExhausted);
        }
        let kind = command.intent.kind();
        let campaign_id = command.intent.campaign_id().to_owned();
        let (state, decision) = match command.intent {
            OperatorIntent::Start { contract } => (start_in(&tx, contract)?, None),
            OperatorIntent::Continue {
                expected_version, ..
            } => (
                transition_in(
                    &tx,
                    &campaign_id,
                    expected_version,
                    Some(WakeKind::Operator),
                    |mut state| {
                        state.continue_admission();
                        Ok(state)
                    },
                )?,
                None,
            ),
            OperatorIntent::Pause {
                expected_version, ..
            } => (
                transition_in(&tx, &campaign_id, expected_version, None, |mut state| {
                    state.pause();
                    Ok(state)
                })?,
                None,
            ),
            OperatorIntent::Cancel {
                expected_version, ..
            } => (
                transition_in(&tx, &campaign_id, expected_version, None, |mut state| {
                    state.cancel();
                    Ok(state)
                })?,
                None,
            ),
            OperatorIntent::IssueDecision {
                expected_version,
                identity,
                ..
            } => {
                let state = issue_in(&tx, &campaign_id, expected_version, &identity)?;
                let decision = decision_outcome(&state, &identity);
                (state, decision)
            }
            OperatorIntent::ResolveDecision {
                expected_version,
                identity,
                work_identity,
                policy_identity,
                candidate_identity,
                answer,
                ..
            } => {
                let state = transition_in(
                    &tx,
                    &campaign_id,
                    expected_version,
                    Some(WakeKind::Review),
                    |mut state| {
                        state.resolve_decision(
                            &identity,
                            work_identity.as_ref(),
                            &policy_identity,
                            candidate_identity.as_ref(),
                            &answer,
                        )?;
                        Ok(state)
                    },
                )?;
                let decision = decision_outcome(&state, &identity);
                (state, decision)
            }
        };
        let store_version: i64 = tx
            .query_row(
                "SELECT version FROM implementation_campaign WHERE campaign_id=?1",
                params![campaign_id],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let outcome = OperatorOutcome {
            request_id: command.request_id,
            actor: command.actor,
            command: kind,
            campaign_id,
            store_version: u64::try_from(store_version).map_err(storage)?,
            admission: AdmissionState::of(&state),
            decision,
        };
        let bytes = serde_json::to_vec(&outcome).map_err(storage)?;
        if bytes.len() > MAX_OUTCOME_BYTES {
            return Err(OperatorFailure::Storage(
                "operator outcome exceeds its bound".into(),
            ));
        }
        tx.execute(
            "INSERT INTO implementation_campaign_command(request_id,campaign_id,fingerprint,outcome) VALUES(?1,?2,?3,?4)",
            params![outcome.request_id, outcome.campaign_id, fingerprint, bytes],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(AppliedCommand {
            outcome,
            replayed: false,
        })
    }
}
