use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kiln_core::types::ExternalRef;
use kiln_provider_api::{InfrastructureFailure, PortFailure};

use crate::protocol::CallbackState;
use crate::transport::AspenServiceAck;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalServiceFault {
    None,
    TimeoutBeforeCallback,
    CancelBeforeCallback,
    UnavailableBeforeCallback,
    DisconnectAfterCommit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackInvocationError {
    Denied(String),
    TimedOut,
    Cancelled,
    Unavailable,
}

pub trait HostedCallback {
    fn invoke(
        &mut self,
        request: &[u8],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, CallbackInvocationError>;
}

#[derive(Clone, Debug)]
pub(super) struct LocalServiceState {
    pub(super) callback_state: CallbackState,
    pub(super) acknowledgements: BTreeMap<ExternalRef, AspenServiceAck>,
    pub(super) next_fault: LocalServiceFault,
}

#[derive(Clone, Debug)]
pub struct LocalServiceJournal {
    pub(super) state: Arc<Mutex<LocalServiceState>>,
}

impl LocalServiceJournal {
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(LocalServiceState {
                callback_state: CallbackState::initial(generation),
                acknowledgements: BTreeMap::new(),
                next_fault: LocalServiceFault::None,
            })),
        }
    }

    pub fn set_next_fault(&self, fault: LocalServiceFault) -> Result<(), PortFailure> {
        let mut state = self.state.lock().map_err(|_| poisoned_failure())?;
        state.next_fault = fault;
        Ok(())
    }

    pub fn callback_state(&self) -> Result<CallbackState, PortFailure> {
        let state = self.state.lock().map_err(|_| poisoned_failure())?;
        Ok(state.callback_state.clone())
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn poisoned_failure() -> PortFailure {
    PortFailure::Infrastructure(infrastructure(
        "aspen_service_lock",
        "local Aspen service journal lock is unavailable",
    ))
}
