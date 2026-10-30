use std::path::{Path, PathBuf};

use super::profile::{Step, WorkerProfile};

/// Admission occurs before stream helpers or job processes are started.
pub fn command(
    profile: &WorkerProfile,
    step: &Step,
    checkout: &Path,
    directory: &Path,
    usage: &Path,
) -> Result<(PathBuf, Vec<String>), String> {
    match &profile.execution {
        Some(policy) => {
            crate::execution::preflight(policy)?;
            // Resolve the private checkout once; do not mount a symlink chosen by
            // source content or expose a parent directory containing credentials.
            let source =
                std::fs::canonicalize(checkout).map_err(|_| "isolation checkout unavailable")?;
            let root = std::fs::canonicalize(&profile.work_root)
                .map_err(|_| "isolation work root unavailable")?;
            if !source.starts_with(&root) || source == root {
                return Err("isolation checkout is not a private work-root child".into());
            }
            let working = std::fs::canonicalize(directory)
                .map_err(|_| super::source::PROJECT_ROOT_MISSING)?;
            if !working.starts_with(&source) {
                return Err(super::source::PROJECT_ROOT_ESCAPES_CHECKOUT.into());
            }
            for sensitive in [&profile.token_file, &profile.repository_path] {
                let sensitive = std::fs::canonicalize(sensitive)
                    .map_err(|_| "isolation sensitive path unavailable")?;
                if sensitive.starts_with(&source)
                    || policy
                        .read_only_mounts
                        .iter()
                        .any(|mount| sensitive.starts_with(mount))
                {
                    return Err("isolation mount overlaps a sensitive host path".into());
                }
            }
            let (observer, arguments) = super::streaming::entry_command()?;
            let metrics = usage.parent().ok_or("worker metrics parent missing")?;
            policy.command(
                &source,
                &working,
                &step.program,
                &step.argv,
                &step.environment,
                crate::execution::ObservationMount {
                    executable: &observer,
                    arguments: &arguments,
                    directory: metrics,
                },
            )
        }
        None => Ok((step.program.clone(), step.argv.clone())),
    }
}
