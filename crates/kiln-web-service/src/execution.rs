use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

// r[impl kiln.web.developer_loop.workers]
/// Reviewed Linux namespace policy. This is defense in depth for trusted CI,
/// not a claim that arbitrary hostile programs are confined.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IsolationPolicy {
    pub bubblewrap: PathBuf,
    pub probe_program: PathBuf,
    pub read_only_mounts: Vec<PathBuf>,
    pub network: NetworkPolicy,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
    pub maximum_file_bytes: u64,
    pub maximum_open_files: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Deny,
    Host,
}

pub(crate) struct ObservationMount<'a> {
    pub executable: &'a Path,
    pub arguments: &'a [OsString],
    pub directory: &'a Path,
}

impl IsolationPolicy {
    pub fn admit(&self) -> Result<(), String> {
        let clean = |p: &Path| {
            p.is_absolute()
                && p.components()
                    .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
        };
        let mut mounts = std::collections::BTreeSet::new();
        if !clean(&self.bubblewrap)
            || !clean(&self.probe_program)
            || !self
                .read_only_mounts
                .iter()
                .any(|mount| self.probe_program.starts_with(mount))
            || self.read_only_mounts.is_empty()
            || self.read_only_mounts.len() > 16
            || self.read_only_mounts.iter().any(|p| {
                !clean(p)
                    || !["/nix/store", "/usr", "/bin", "/lib", "/lib64"]
                        .iter()
                        .any(|root| p == Path::new(root))
                    || !mounts.insert(p)
            })
            || self.cpu_seconds == 0
            || self.cpu_seconds > 86_400
            || self.address_space_bytes < 16_777_216
            || self.address_space_bytes > 1_099_511_627_776
            || self.maximum_file_bytes == 0
            || self.maximum_file_bytes > 1_099_511_627_776
            || self.maximum_open_files < 16
            || self.maximum_open_files > 65_536
        {
            return Err("isolation policy outside reviewed bounds".into());
        }
        Ok(())
    }

    pub(crate) fn command(
        &self,
        checkout: &Path,
        directory: &Path,
        program: &Path,
        argv: &[String],
        environment: &std::collections::BTreeMap<String, String>,
        observation: ObservationMount<'_>,
    ) -> Result<(PathBuf, Vec<String>), String> {
        self.admit()?;
        if !directory.starts_with(checkout) {
            return Err("isolated working directory is outside the checkout".into());
        }
        if !self.read_only_mounts.iter().any(|p| program.starts_with(p)) {
            return Err("isolated executable is not in a reviewed runtime mount".into());
        }
        let mut args = self.namespace_arguments();
        args.extend([
            "--ro-bind".into(),
            text(observation.executable)?,
            "/run/kiln-step-observer".into(),
            "--bind".into(),
            text(observation.directory)?,
            "/run/kiln-metrics".into(),
        ]);
        for (name, value) in environment {
            args.extend(["--setenv".into(), name.clone(), value.clone()]);
        }
        args.extend([
            "--bind".into(),
            text(checkout)?,
            text(checkout)?,
            "--chdir".into(),
            text(directory)?,
            "--".into(),
            "/run/kiln-step-observer".into(),
        ]);
        for argument in observation.arguments {
            args.push(
                argument
                    .to_str()
                    .ok_or("worker observer argument encoding invalid")?
                    .to_owned(),
            );
        }
        args.extend([
            "--measure-child".into(),
            "/run/kiln-metrics/usage".into(),
            serde_json::to_string(self).map_err(|_| "worker resource policy encoding failed")?,
            "--".into(),
            text(program)?,
        ]);
        args.extend_from_slice(argv);
        Ok((self.bubblewrap.clone(), args))
    }

    fn namespace_arguments(&self) -> Vec<String> {
        let mut args = vec![
            "--die-with-parent".into(),
            "--unshare-all".into(),
            "--clearenv".into(),
            "--cap-drop".into(),
            "ALL".into(),
        ];
        if self.network == NetworkPolicy::Host {
            args.push("--share-net".into());
        }
        for mount in &self.read_only_mounts {
            args.extend([
                "--ro-bind".into(),
                mount.to_string_lossy().into_owned(),
                mount.to_string_lossy().into_owned(),
            ]);
        }
        args.extend([
            "--proc".into(),
            "/proc".into(),
            "--dev".into(),
            "/dev".into(),
            "--tmpfs".into(),
            "/tmp".into(),
            "--tmpfs".into(),
            "/run".into(),
            "--tmpfs".into(),
            "/home".into(),
        ]);
        args
    }
}

fn text(path: &Path) -> Result<String, String> {
    path.to_str()
        .filter(|s| !s.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| "isolation path encoding invalid".into())
}

/// Preflight runs only the reviewed bubblewrap backend and an explicit benign
/// probe executable, never a job. Failures are terminal without fallback.
pub fn preflight(policy: &IsolationPolicy) -> Result<(), String> {
    use bounded_exec::{
        CommandSpec, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
        TerminationScope,
    };
    policy.admit()?;
    let program = &policy.probe_program;
    let mut args = policy.namespace_arguments();
    args.extend(["--".into(), text(program)?]);
    let result = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program: policy.bubblewrap.clone(),
            args: args.into_iter().map(OsString::from).collect(),
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: 5_000,
            stdin_max_bytes: 1,
            stdout_max_bytes: 4096,
            stderr_max_bytes: 4096,
            poll_interval_ms: 10,
            teardown_timeout_ms: 2000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|_| "isolation preflight policy invalid")?,
    })
    .map_err(|_| "isolation backend unavailable")?;
    if result.exit_code != Some(0) || result.completion != bounded_exec::Completion::Exited {
        return Err("isolation preflight unavailable; job execution denied".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> IsolationPolicy {
        IsolationPolicy {
            bubblewrap: "/missing/bwrap".into(),
            probe_program: "/usr/bin/true".into(),
            read_only_mounts: vec!["/usr".into()],
            network: NetworkPolicy::Deny,
            cpu_seconds: 60,
            address_space_bytes: 268_435_456,
            maximum_file_bytes: 16_777_216,
            maximum_open_files: 64,
        }
    }
    #[test]
    fn reviewed_runtime_authority_is_admitted_without_unrelated_host_paths() {
        let p = policy();
        assert!(p.admit().is_ok());
        let mut host = p.clone();
        host.network = NetworkPolicy::Host;
        assert!(host.admit().is_ok());
        let mut credentials = p;
        credentials.read_only_mounts.push("/etc".into());
        assert!(credentials.admit().is_err());
    }
    #[test]
    fn unrelated_mounts_and_unavailable_backend_fail_closed() {
        let mut p = policy();
        assert!(preflight(&p).is_err());
        p.read_only_mounts.push("/home".into());
        assert!(p.admit().is_err());
        assert!(
            policy()
                .command(
                    Path::new("/tmp/source"),
                    Path::new("/tmp/source"),
                    Path::new("/home/token"),
                    &[],
                    &std::collections::BTreeMap::new(),
                    ObservationMount {
                        executable: Path::new("/observer"),
                        arguments: &[],
                        directory: Path::new("/metrics")
                    }
                )
                .is_err()
        );
    }
}
