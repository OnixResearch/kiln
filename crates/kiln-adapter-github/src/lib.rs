pub mod events;
pub mod job;
pub mod profile;
pub mod replay;
pub mod secret;
pub mod server;
pub mod service;
pub mod status;

use kiln_core::types::Diagnostic;
use kiln_provider_api::{InfrastructureFailure, PortFailure};

pub(crate) fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

pub(crate) fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests;
