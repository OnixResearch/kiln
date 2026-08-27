use kiln_core::remote::RemoteAction;
use kiln_core::types::ExternalRef;
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, RemoteExecutionPort, RemoteLifecyclePort,
    RemoteTransportObservation,
};
use serde::{Deserialize, Serialize};

use crate::profile::AdmittedAspenRuntimeProfile;

pub const REMOTE_TRANSPORT_SCHEMA: &str = "kiln.aspen-runtime.remote-transport.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AspenRemoteRequest {
    Upload(RemoteAction),
    Execute(RemoteAction),
    Cancel(ExternalRef),
    Poll(ExternalRef),
    Wait(ExternalRef),
    Download {
        output: ExternalRef,
        max_bytes: usize,
    },
    ReadLog {
        log: ExternalRef,
        max_bytes: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AspenRemoteOutcome {
    Uploaded,
    Observation(RemoteTransportObservation),
    Bytes(Vec<u8>),
    DisconnectedAfterAcceptance { operation_id: Option<ExternalRef> },
    TimedOut,
    Cancelled,
    Unavailable,
    Malformed,
}

pub trait AspenRemoteTransportPort {
    fn dispatch(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenRemoteRequest,
    ) -> AspenRemoteOutcome;
}

#[derive(Clone, Debug)]
pub struct AspenRemoteAdapter<T> {
    profile: AdmittedAspenRuntimeProfile,
    transport: T,
}

impl<T> AspenRemoteAdapter<T> {
    #[must_use]
    pub const fn new(profile: AdmittedAspenRuntimeProfile, transport: T) -> Self {
        Self { profile, transport }
    }

    fn dispatch_observation(
        &mut self,
        request: AspenRemoteRequest,
    ) -> PortResult<RemoteTransportObservation>
    where
        T: AspenRemoteTransportPort,
    {
        match self.transport.dispatch(&self.profile, &request) {
            AspenRemoteOutcome::Observation(observation) => Ok(observation),
            AspenRemoteOutcome::DisconnectedAfterAcceptance { operation_id } => {
                let action = action_ref(&request)?;
                Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                    action,
                    operation: operation_id,
                })
            }
            AspenRemoteOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "aspen_remote_timeout",
                "Aspen remote operation exceeded its admitted deadline",
            ))),
            AspenRemoteOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
                "aspen_remote_cancelled",
                "Aspen remote operation was cancelled",
            ))),
            AspenRemoteOutcome::Unavailable => Err(PortFailure::Infrastructure(infrastructure(
                "aspen_remote_unavailable",
                "selected Aspen remote fabric port is unavailable and no fallback is allowed",
            ))),
            AspenRemoteOutcome::Uploaded
            | AspenRemoteOutcome::Bytes(_)
            | AspenRemoteOutcome::Malformed => Err(domain(
                "aspen_remote_malformed",
                "aspen.remote.response",
                "Aspen remote fabric returned an outcome for a different operation class",
            )),
        }
    }

    fn dispatch_bytes(&mut self, request: AspenRemoteRequest, maximum: usize) -> PortResult<Vec<u8>>
    where
        T: AspenRemoteTransportPort,
    {
        if maximum == 0 || maximum > self.profile.profile.bounds.max_content_bytes {
            return Err(domain(
                "aspen_remote_bound",
                "aspen.remote.max_bytes",
                "remote content bound must fit the admitted Aspen profile",
            ));
        }
        match self.transport.dispatch(&self.profile, &request) {
            AspenRemoteOutcome::Bytes(bytes) if bytes.len() <= maximum => Ok(bytes),
            AspenRemoteOutcome::Bytes(_) => Err(domain(
                "aspen_remote_bound",
                "aspen.remote.bytes",
                "remote content exceeds its admitted byte bound",
            )),
            AspenRemoteOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "aspen_remote_timeout",
                "Aspen remote content operation exceeded its admitted deadline",
            ))),
            AspenRemoteOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
                "aspen_remote_cancelled",
                "Aspen remote content operation was cancelled",
            ))),
            AspenRemoteOutcome::Unavailable => Err(PortFailure::Infrastructure(infrastructure(
                "aspen_remote_unavailable",
                "selected Aspen content port is unavailable and no fallback is allowed",
            ))),
            AspenRemoteOutcome::Uploaded
            | AspenRemoteOutcome::Observation(_)
            | AspenRemoteOutcome::DisconnectedAfterAcceptance { .. }
            | AspenRemoteOutcome::Malformed => Err(domain(
                "aspen_remote_malformed",
                "aspen.remote.response",
                "Aspen remote content operation returned a malformed response",
            )),
        }
    }
}

impl<T: AspenRemoteTransportPort> RemoteExecutionPort for AspenRemoteAdapter<T> {
    fn upload(&mut self, action: &RemoteAction) -> PortResult<()> {
        match self
            .transport
            .dispatch(&self.profile, &AspenRemoteRequest::Upload(action.clone()))
        {
            AspenRemoteOutcome::Uploaded => Ok(()),
            AspenRemoteOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "aspen_remote_timeout",
                "Aspen remote upload exceeded its admitted deadline",
            ))),
            AspenRemoteOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
                "aspen_remote_cancelled",
                "Aspen remote upload was cancelled",
            ))),
            AspenRemoteOutcome::Unavailable => Err(PortFailure::Infrastructure(infrastructure(
                "aspen_remote_unavailable",
                "selected Aspen content port is unavailable and no fallback is allowed",
            ))),
            AspenRemoteOutcome::Observation(_)
            | AspenRemoteOutcome::Bytes(_)
            | AspenRemoteOutcome::DisconnectedAfterAcceptance { .. }
            | AspenRemoteOutcome::Malformed => Err(domain(
                "aspen_remote_upload",
                "aspen.remote.upload",
                "Aspen remote upload returned an invalid outcome",
            )),
        }
    }

    fn execute(&mut self, action: &RemoteAction) -> PortResult<RemoteTransportObservation> {
        self.dispatch_observation(AspenRemoteRequest::Execute(action.clone()))
    }

    fn cancel(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        operation.validate().map_err(PortFailure::Domain)?;
        self.dispatch_observation(AspenRemoteRequest::Cancel(operation.clone()))
    }
}

impl<T: AspenRemoteTransportPort> RemoteLifecyclePort for AspenRemoteAdapter<T> {
    fn poll(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        operation.validate().map_err(PortFailure::Domain)?;
        self.dispatch_observation(AspenRemoteRequest::Poll(operation.clone()))
    }

    fn wait(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        operation.validate().map_err(PortFailure::Domain)?;
        self.dispatch_observation(AspenRemoteRequest::Wait(operation.clone()))
    }

    fn download(&mut self, output: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>> {
        output.validate().map_err(PortFailure::Domain)?;
        self.dispatch_bytes(
            AspenRemoteRequest::Download {
                output: output.clone(),
                max_bytes,
            },
            max_bytes,
        )
    }

    fn read_log(&mut self, log: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>> {
        log.validate().map_err(PortFailure::Domain)?;
        if max_bytes > self.profile.profile.bounds.max_log_bytes {
            return Err(domain(
                "aspen_remote_log_bound",
                "aspen.remote.log",
                "remote log request exceeds the admitted log bound",
            ));
        }
        self.dispatch_bytes(
            AspenRemoteRequest::ReadLog {
                log: log.clone(),
                max_bytes,
            },
            max_bytes,
        )
    }
}

fn action_ref(request: &AspenRemoteRequest) -> PortResult<ExternalRef> {
    match request {
        AspenRemoteRequest::Execute(action) | AspenRemoteRequest::Upload(action) => {
            let bytes = serde_json::to_vec(action).map_err(|_| {
                domain(
                    "aspen_remote_encoding",
                    "aspen.remote.action",
                    "remote action identity encoding failed",
                )
            })?;
            let digest = blake3::hash(&bytes).to_hex().to_string();
            ExternalRef::new("molten", "remote-action-blake3", &digest).map_err(PortFailure::Domain)
        }
        AspenRemoteRequest::Cancel(operation)
        | AspenRemoteRequest::Poll(operation)
        | AspenRemoteRequest::Wait(operation) => Ok(operation.clone()),
        AspenRemoteRequest::Download { output, .. } => Ok(output.clone()),
        AspenRemoteRequest::ReadLog { log, .. } => Ok(log.clone()),
    }
}

fn domain(code: &str, path: &'static str, message: &str) -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(code, path, message))
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}
