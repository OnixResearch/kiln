use cob_git2 as git2;
use kiln_core::types::Diagnostic;
use kiln_provider_api::{InfrastructureFailure, PortFailure};

pub(super) fn git_failure(_error: git2::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_source_git".to_string(),
        message: "Radicle source repository operation failed".to_string(),
    })
}

pub(super) fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

pub(super) fn platform_bound_failure() -> PortFailure {
    domain(
        "radicle_source_platform_bound",
        "radicle_source.limits",
        "Radicle source bound exceeds this platform",
    )
}
