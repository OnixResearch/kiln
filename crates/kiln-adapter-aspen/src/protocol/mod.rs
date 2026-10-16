use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use kiln_core::domain::Outcome;
use kiln_core::types::{ContentId, Diagnostic, EffectId, ExternalRef, ProviderProfileId, RunId};
use kiln_provider_api::TriggerRequest;
use serde::{Deserialize, Serialize};

mod codec;
mod native;
mod provider;

pub use codec::*;
pub use native::*;
pub use provider::*;

pub const CALLBACK_REQUEST_SCHEMA: &str = "kiln.aspen-extension.callback-request.v2";
pub const CALLBACK_OUTCOME_SCHEMA: &str = "kiln.aspen-extension.callback-outcome.v2";
pub const CALLBACK_STATE_SCHEMA: &str = "kiln.aspen-extension.state.v2";
pub const CALLBACK_CHECKPOINT_SCHEMA: &str = "kiln.aspen-extension.checkpoint.v2";
pub const CALLBACK_STATUS_SCHEMA: &str = "kiln.aspen-extension.status.v2";
pub const CALLBACK_EFFECT_SCHEMA: &str = "kiln.aspen-extension.effect.v2";
pub const CALLBACK_COMPLETION_SCHEMA: &str = "kiln.aspen-extension.completion.v2";
pub const HOSTED_CALLBACK_INPUT_SCHEMA: &str = "kiln.aspen-extension.hosted-input.v2";
pub const CALLBACK_REQUEST_RECORD: &str = "kiln-aspen-callback-request-v2";
pub const HOSTED_CALLBACK_INPUT_RECORD: &str = "kiln-aspen-hosted-input-v2";
pub const CALLBACK_OUTCOME_RECORD: &str = "kiln-aspen-callback-outcome-v2";
const HOSTED_RUN_ID_PREFIX: &str = "kiln-aspen-run-";

// r[impl kiln.aspen_host.transport]
pub fn hosted_run_id(trigger_identity: &ContentId) -> Result<RunId, Diagnostic> {
    let digest =
        blake3::hash(format!("kiln.aspen.run.v1\0{}", trigger_identity.as_str()).as_bytes())
            .to_hex()
            .to_string();
    RunId::new(format!("{HOSTED_RUN_ID_PREFIX}{digest}"))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspenEffectKind {
    DurableState,
    ContentStore,
    BoundedProcess,
    TransportSession,
    Timer,
    Schedule,
    Evidence,
}

impl AspenEffectKind {
    #[must_use]
    pub const fn port_purpose(self) -> &'static str {
        match self {
            Self::DurableState => "durability",
            Self::ContentStore => "content",
            Self::BoundedProcess => "execution",
            Self::TransportSession => "transport",
            Self::Timer => "time",
            Self::Schedule => "scheduling",
            Self::Evidence => "evidence",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackEffectPlan {
    pub schema: String,
    pub effect_id: EffectId,
    pub kind: AspenEffectKind,
    pub provider_profile: ProviderProfileId,
    pub operation_id: ExternalRef,
    pub idempotency_key: ExternalRef,
    pub request_identity: ContentId,
    pub authority_ref: String,
    pub resource_ref: String,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallbackEffectTerminal {
    Success,
    Failure,
    Denied,
    TimedOut,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackEffectCompletion {
    pub schema: String,
    pub effect_id: EffectId,
    pub operation_id: ExternalRef,
    pub generation: u64,
    pub terminal: CallbackEffectTerminal,
    pub observation_ref: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CallbackCommand {
    SubmitTrigger {
        trigger: Box<TriggerRequest>,
        trigger_identity: ContentId,
        idempotency_key: ExternalRef,
    },
    ApplyProviderObservation {
        effect_id: EffectId,
        provider_profile: ProviderProfileId,
        observation_ref: ExternalRef,
        outcome: Outcome,
    },
    CompleteEffect(CallbackEffectCompletion),
    Checkpoint,
    Recover {
        checkpoint_ref: ExternalRef,
    },
    Status,
    Drain,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedRunPhase {
    Accepted,
    DispatchPending,
    Running,
    Completed,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostedRunState {
    pub run_id: RunId,
    pub trigger_identity: ContentId,
    pub phase: HostedRunPhase,
    pub provider_effect: Option<EffectId>,
    pub provider_observation: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackState {
    pub schema: String,
    pub generation: u64,
    pub revision: u64,
    pub runs: BTreeMap<String, HostedRunState>,
    pub unresolved_effects: BTreeMap<String, CallbackEffectPlan>,
    pub draining: bool,
    pub stopped: bool,
}

impl CallbackState {
    #[must_use]
    pub fn initial(generation: u64) -> Self {
        Self {
            schema: CALLBACK_STATE_SCHEMA.to_string(),
            generation,
            revision: 0,
            runs: BTreeMap::new(),
            unresolved_effects: BTreeMap::new(),
            draining: false,
            stopped: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostedCallbackInput {
    pub schema: String,
    pub protocol_schema: String,
    pub profile_identity: ContentId,
    pub operation_id: ExternalRef,
    pub authority_ref: String,
    pub policy_ref: String,
    pub resource_ref: String,
    pub command: CallbackCommand,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackRequest {
    pub schema: String,
    pub protocol_schema: String,
    pub profile_identity: ContentId,
    pub generation: u64,
    pub sequence: u64,
    pub operation_id: ExternalRef,
    pub authority_ref: String,
    pub policy_ref: String,
    pub resource_ref: String,
    pub deadline_tick: u64,
    pub prior_state: CallbackState,
    pub command: CallbackCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallbackDecision {
    Accepted,
    Rejected,
    Replay,
    Reconciled,
    Checkpointed,
    Recovered,
    Status,
    Draining,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackStatus {
    pub schema: String,
    pub generation: u64,
    pub revision: u64,
    pub run_count: usize,
    pub unresolved_effect_count: usize,
    pub draining: bool,
    pub stopped: bool,
    pub claim_level: String,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallbackOutcome {
    pub schema: String,
    pub protocol_schema: String,
    pub generation: u64,
    pub operation_id: ExternalRef,
    pub decision: CallbackDecision,
    pub state: CallbackState,
    pub effects: Vec<CallbackEffectPlan>,
    pub checkpoint_ref: Option<ExternalRef>,
    pub status: CallbackStatus,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenNativeOutcomeProjection {
    pub schema: String,
    pub state_ref: ContentId,
    pub checkpoint_ref: Option<ExternalRef>,
    pub output_ref: ContentId,
    pub effect_request_refs: Vec<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    BoundExceeded,
    Parse(String),
    NonCanonical,
    Identity,
    Shape,
    Schema,
    Json(String),
}

impl Display for ProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BoundExceeded => {
                formatter.write_str("callback frame exceeds its named byte bound")
            }
            Self::Parse(message) => write!(formatter, "packed Preserves parse failed: {message}"),
            Self::NonCanonical => {
                formatter.write_str("callback frame is not canonical packed Preserves")
            }
            Self::Identity => {
                formatter.write_str("callback value identity does not match its bytes")
            }
            Self::Shape => formatter.write_str("callback frame has an unsupported Preserves shape"),
            Self::Schema => {
                formatter.write_str("callback frame schema or protocol version is unsupported")
            }
            Self::Json(message) => write!(formatter, "callback payload decode failed: {message}"),
        }
    }
}

impl std::error::Error for ProtocolError {}
