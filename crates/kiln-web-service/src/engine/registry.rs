use crate::profile::ServiceProfile;

use super::{identity, storage::Store};

// r[impl kiln.web.projects.extension]
fn admit_extension(previous: &ServiceProfile, next: &ServiceProfile) -> Result<(), String> {
    previous.admit()?;
    next.admit()?;
    let mut bounds = previous.bounds.clone();
    bounds.max_projects = next.bounds.max_projects;
    if next.projects.len() <= previous.projects.len()
        || next.bounds.max_projects < previous.bounds.max_projects
        || next.schema != previous.schema
        || next.service_id != previous.service_id
        || next.listen_address != previous.listen_address
        || next.origin != previous.origin
        || next.tls.certificate_path != previous.tls.certificate_path
        || next.tls.private_key_path != previous.tls.private_key_path
        || next.tls.min_version != previous.tls.min_version
        || next.operator_token_file != previous.operator_token_file
        || next.worker_token_file != previous.worker_token_file
        || next.state_directory != previous.state_directory
        || next.bounds != bounds
    {
        return Err("project extension may only append projects and increase max_projects".into());
    }
    for (before, after) in previous.projects.iter().zip(&next.projects) {
        if identity(before)? != identity(after)? {
            return Err("project extension must preserve existing projects in order".into());
        }
    }
    Ok(())
}

/// Extend an existing stopped service's project allowlist without rewriting its history.
///
/// The exact previous profile and all existing projects remain authority. Only
/// appended projects and an increased project-count bound are admitted. Projects
/// are not enrolled here; ordinary authenticated enrollment remains required.
// r[impl kiln.web.projects.extension]
pub fn extend_projects(previous: &ServiceProfile, next: &ServiceProfile) -> Result<(), String> {
    admit_extension(previous, next)?;
    let previous_identity = identity(previous)?;
    let next_identity = identity(next)?;
    let (mut store, state, _lock) = Store::open_existing(previous)?;
    if state.runs.values().any(|run| run.phase != "terminal") {
        return Err("project extension requires all existing executions to be terminal".into());
    }
    store.extend_binding(previous_identity.as_str(), next_identity.as_str())
}
