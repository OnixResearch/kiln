#![allow(
    unknown_lints,
    non_trait_imports,
    reason = "the closed wire schema imports producer contract names so each field keeps its exact compatibility type"
)]

#[path = "model/bytes.rs"]
mod encoded;

use lattice_workflow_contract::CapabilityProfile;
use lattice_workflow_contract::ControlRequest;
use lattice_workflow_contract::ExecutionObservation;
use lattice_workflow_contract::Identity;
use lattice_workflow_contract::LogReadRequest;
use lattice_workflow_contract::WorkflowAccepted;
use lattice_workflow_contract::WorkflowRequest;
use serde::Deserialize;
use serde::Serialize;

pub const TRANSPORT_PROFILE_SCHEMA: &str = "lattice.workflow-transport-profile.v1";
pub const EXCHANGE_REQUEST_SCHEMA: &str = "lattice.workflow-exchange-request.v1";
pub const EXCHANGE_RESPONSE_SCHEMA: &str = "lattice.workflow-exchange-response.v1";
pub const MAXIMUM_FRAME_BYTES: u32 = 33_554_432;
pub const MAXIMUM_LOG_BYTES: u32 = 16_777_216;
pub const MAXIMUM_CONNECTIONS: u32 = 65_536;
pub const MAXIMUM_DEADLINE_MILLISECONDS: u64 = 300_000;
pub const MAXIMUM_ALLOWED_USERS: u32 = 64;
pub const FRAME_HEADER_BYTES: usize = 4;
pub const SOCKET_PERMISSION_MODE: u32 = 0o600;

pub const REQUIRED_NON_CLAIMS: [&str; 5] = [
    "transport admission does not prove workflow correctness",
    "transport peer identity does not prove peer intent",
    "transport acceptance does not prove an operation executed",
    "transport response does not prove an external effect succeeded",
    "transport checks do not establish production deployment",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportLimits {
    pub maximum_frame_bytes: u32,
    pub maximum_log_bytes: u32,
    pub maximum_connections: u32,
    pub read_deadline_milliseconds: u64,
    pub write_deadline_milliseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerPolicy {
    pub allowed_user_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportProfile {
    pub schema: String,
    pub limits: TransportLimits,
    pub peers: PeerPolicy,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "payload", rename_all = "snake_case")]
pub enum ExchangeCommand {
    Dispatch {
        profile: CapabilityProfile,
        request: WorkflowRequest,
    },
    // r[impl lattice.workflow_interchange.transport_dispatch_recovery]
    RecoverDispatch {
        profile: CapabilityProfile,
        request: WorkflowRequest,
    },
    Observe {
        profile: CapabilityProfile,
        request: WorkflowRequest,
        accepted: WorkflowAccepted,
    },
    Control {
        profile: CapabilityProfile,
        request: WorkflowRequest,
        accepted: WorkflowAccepted,
        current_sequence: u64,
        control: ControlRequest,
    },
    ReadLog {
        profile: CapabilityProfile,
        request: WorkflowRequest,
        accepted: WorkflowAccepted,
        log_read: LogReadRequest,
    },
    Reconcile {
        profile: CapabilityProfile,
        request: WorkflowRequest,
        accepted: WorkflowAccepted,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRequest {
    pub schema: String,
    pub identity: Identity,
    pub command: ExchangeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ExchangeSuccess {
    Accepted(WorkflowAccepted),
    Observation(ExecutionObservation),
    Log {
        observation: ExecutionObservation,
        #[serde(with = "encoded")]
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionCode {
    InvalidSchema,
    InvalidIdentity,
    InvalidContract,
    InvalidOperationResult,
    FrameTooLarge,
    LogTooLarge,
    PeerDenied,
    HandlerUnavailable,
    NotFound,
    EncodingFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rejection {
    pub code: RejectionCode,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", content = "payload", rename_all = "snake_case")]
pub enum ExchangeResult {
    Success(Box<ExchangeSuccess>),
    Rejected(Rejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeResponse {
    pub schema: String,
    pub identity: Identity,
    pub request_identity: Identity,
    pub result: ExchangeResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    ProfileSchema,
    ProfileLimit,
    PeerPolicy,
    RequestSchema,
    RequestIdentity,
    Contract,
    ResponseSchema,
    ResponseIdentity,
    ResponseRequest,
    OperationResult,
    LogBound,
    LogLinkage,
    Encoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerError {
    Unavailable,
    NotFound,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientFailure {
    InvalidProfile,
    InvalidRequest,
    ConnectFailed,
    NotAccepted,
    UnknownAfterWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerFailure {
    InvalidProfile,
    BindFailed,
    PermissionFailed,
    AcceptFailed,
    PeerObservationFailed,
    PeerDenied,
    ReadFailed,
    WriteFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServeReport {
    pub accepted_connections: u32,
    pub completed_exchanges: u32,
    pub rejected_peers: u32,
    pub failed_exchanges: u32,
}
