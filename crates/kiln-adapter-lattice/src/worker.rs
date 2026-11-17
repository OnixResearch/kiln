//! Real bounded transport to the explicitly adopted Lattice implementation CLI.
use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use kiln_core::implementation::{Enrollment, ImplementationDispatchPlan};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[path = "worker_metrics.rs"]
mod metrics;

const MAX_OMP_OBSERVATION_BYTES: u64 = 16 * 1024;

fn read_metrics_artifact(
    state: &Path,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    response: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    // This is the native producer's per-attempt directory identity. An artifact
    // pointer from another attempt cannot supply this attempt's observations.
    let key = state.join(format!("{}-{}", attempt.identity.as_str(), attempt.fence));
    let expected = state
        .join(
            blake3::hash(key.as_os_str().as_encoded_bytes())
                .to_hex()
                .as_str(),
        )
        .join("omp-observation.json");
    let path = response
        .pointer("/artifacts/omp_observation")
        .and_then(serde_json::Value::as_str)
        .map(Path::new)
        .ok_or("worker metrics artifact is unavailable")?;
    if path != expected
        || std::fs::canonicalize(path).map_err(|error| error.to_string())? != expected
    {
        return Err("worker metrics artifact belongs to another attempt or path".into());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_OMP_OBSERVATION_BYTES {
        return Err("worker metrics artifact is not a bounded ordinary file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_OMP_OBSERVATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_OMP_OBSERVATION_BYTES {
        return Err("worker metrics artifact exceeds its bound".into());
    }
    Ok(bytes)
}

const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
fn transport_profile_mode_matches(
    schema: &str,
    native_request_profile: Option<&ContentId>,
) -> bool {
    matches!(
        (schema, native_request_profile),
        ("kiln.lattice-implementation-cli.v1", None)
            | ("kiln.lattice-implementation-cli.v2", Some(_))
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerTransportProfile {
    pub schema: String,
    pub lattice_program: PathBuf,
    pub executable_blake3: ContentId,
    pub state_directory: PathBuf,
    pub output_directory: PathBuf,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return Err("transport input is not a bounded ordinary file".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}
fn required(flags: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    flags
        .get(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is required"))
}
fn clock() -> Result<u64, String> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    u64::try_from(ms).map_err(|error| error.to_string())
}
fn program_identity(path: &Path) -> Result<ContentId, String> {
    if !path.is_absolute()
        || !path.starts_with("/nix/store")
        || !std::fs::canonicalize(path)
            .map_err(|error| error.to_string())?
            .starts_with("/nix/store")
    {
        return Err("worker transport executable is not an admitted immutable Nix program".into());
    }
    let mut input = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..count)
                .ok_or("worker executable read boundary invalid")?,
        );
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .map_err(|error| format!("{error:?}"))
}
fn binding<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a serde_json::Value, String> {
    value
        .get(field)
        .ok_or_else(|| format!("worker request lacks {field}"))
}

/// Exact files for the guarded existing implementation-worker consumer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerInvocation {
    pub database: PathBuf,
    pub enrollment: PathBuf,
    pub dispatch_plan: PathBuf,
    pub request: PathBuf,
    pub transport_profile: PathBuf,
    pub admission_observations: Option<PathBuf>,
}

/// Call the same guarded worker path as `kiln-lattice-dispatch` without
/// converting its stdout into completion evidence.
pub fn run_invocation(invocation: &WorkerInvocation) -> Result<String, String> {
    let mut flags = BTreeMap::from([
        (
            "--database".into(),
            invocation.database.display().to_string(),
        ),
        (
            "--enrollment".into(),
            invocation.enrollment.display().to_string(),
        ),
        (
            "--dispatch-plan".into(),
            invocation.dispatch_plan.display().to_string(),
        ),
        (
            "--worker-request".into(),
            invocation.request.display().to_string(),
        ),
        (
            "--worker-transport-profile".into(),
            invocation.transport_profile.display().to_string(),
        ),
    ]);
    if let Some(path) = &invocation.admission_observations {
        flags.insert(
            "--admission-observations".into(),
            path.display().to_string(),
        );
    }
    run(&flags)
}

// r[impl kiln.implementation.current_authority] r[impl kiln.implementation.unknown_barrier]
pub fn run(flags: &BTreeMap<String, String>) -> Result<String, String> {
    let request_path = required(flags, "--worker-request")?;
    let request: serde_json::Value = read_json(&request_path)?;
    let request_bytes = std::fs::read(&request_path).map_err(|error| error.to_string())?;
    let request_id = ContentId::from_blake3_hex(blake3::hash(&request_bytes).to_hex().to_string())
        .map_err(|error| format!("{error:?}"))?;
    let plan: ImplementationDispatchPlan = read_json(&required(flags, "--dispatch-plan")?)?;
    let profile: WorkerTransportProfile =
        read_json(&required(flags, "--worker-transport-profile")?)?;
    let enrollment: Enrollment = read_json(&required(flags, "--enrollment")?)?;
    if program_identity(&profile.lattice_program)? != profile.executable_blake3 {
        return Err("unqualified or changed Lattice implementation executable".into());
    }
    let execution = plan
        .execution
        .as_ref()
        .ok_or("candidate worker requires explicit bound execution, not legacy configured route")?;
    if !transport_profile_mode_matches(&profile.schema, execution.native_request_profile.as_ref()) {
        return Err(
            "transport profile generation does not match the native request profile binding".into(),
        );
    }
    let envelope = binding(&request, "envelope")?;
    let text_matches = |key: &str, expected: &str| {
        envelope.get(key).and_then(serde_json::Value::as_str) == Some(expected)
    };
    if !text_matches("work_id", plan.work_identity.as_str())
        || envelope
            .get("work_version")
            .and_then(serde_json::Value::as_u64)
            != Some(plan.work_revision)
        || !text_matches("attempt_id", plan.attempt_identity.as_str())
        || envelope.get("fence").and_then(serde_json::Value::as_u64) != Some(plan.fence)
        || !text_matches("repository", &plan.repository)
        || !text_matches(
            "base",
            plan.base
                .as_ref()
                .ok_or("exact base required")?
                .native_id
                .as_str(),
        )
        || !text_matches("workspace_id", plan.workspace.identity.as_str())
        || !text_matches(
            "policy_digest",
            execution.requirements.policy_identity.as_str(),
        )
        || !text_matches(
            "worker_profile",
            execution
                .native_request_profile
                .as_ref()
                .map(kiln_core::types::ContentId::as_str)
                .unwrap_or(execution.requirements.needs.profile_revision.as_str()),
        )
        || request.get("network").and_then(serde_json::Value::as_bool)
            != Some(plan.workspace.network)
    {
        return Err(
            "worker request is crossed with admitted work, attempt, workspace or model policy"
                .into(),
        );
    }
    let mut implementer = Implementer::open(&required(flags, "--database")?, enrollment)?;
    let attempt = implementer.authorize_current(&plan.attempt_identity, plan.fence, clock()?)?;
    let ledger = implementer.work()?;
    let work = ledger
        .current()
        .find(|work| work.identity().is_ok_and(|id| id == plan.work_identity))
        .ok_or("work version is not current")?;
    // The task/spec/verification identity is the admitted work, not a worker field.
    if !text_matches("verification_profile", work.verification_profile.as_str()) {
        return Err("verification profile changed".into());
    }
    let acceptance = kiln_core::implementation::acceptance_digest(work)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if !text_matches("task_spec_digest", acceptance.as_str())
        || !work.aliases.iter().any(|source| {
            let kind = match source.kind {
                kiln_core::implementation::WorkSourceKind::GitHubIssue => "github_issue",
                kiln_core::implementation::WorkSourceKind::CairnTask => "cairn_task",
                kiln_core::implementation::WorkSourceKind::CairnArtifact => "cairn_artifact",
            };
            text_matches("source_kind", kind)
                && text_matches("source_id", &source.native_id)
                && text_matches("readiness_snapshot", source.version.as_str())
        })
    {
        return Err("worker source/spec association differs from admitted work".into());
    }
    if request
        .get("allow_unchanged")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        && work.terminal_policy
            != kiln_core::implementation::TerminalPolicy::VerifiedOrAlreadySatisfied
    {
        return Err("work does not admit an unchanged proposal".into());
    }
    if flags
        .get("--recover")
        .is_some_and(|value| matches!(value.as_str(), "true" | "1"))
        && !implementer.dispatch_requires_recovery(&attempt.identity)?
    {
        return Err(
            "recovery requires prior persisted transport intent; no execution was started".into(),
        );
    }
    let reservation = implementer.reserve_dispatch_plan(
        work,
        &attempt.identity,
        attempt.fence,
        clock()?,
        &plan,
    )?;
    let request_binding = implementer.bind_campaign_request(
        work,
        &attempt.identity,
        attempt.fence,
        clock()?,
        &request_bytes,
    )?;
    if request_binding.plan != reservation.plan || request_binding.request_digest != request_id {
        return Err(
            "retained request seal differs from the exact worker bytes and dispatch plan".into(),
        );
    }
    let admission = implementer.admission_snapshot()?;
    if admission.policy.is_none() {
        return Err(
            "bound worker execution requires an explicit owner-grant admission policy".into(),
        );
    }
    let recovery = implementer.dispatch_requires_recovery(&attempt.identity)?;
    if !recovery {
        let item = admission
            .items
            .get(&plan.work_identity)
            .ok_or("queued work is missing")?;
        let expires = request
            .get("expires_at_ms")
            .and_then(serde_json::Value::as_u64)
            .ok_or("worker admission lacks deadline")?;
        let runtime_ms = u64::from(plan.deadline_minutes) * 60 * 1000;
        if expires > item.deadline_ms
            || expires > attempt.lease_until_ms
            || clock()?
                .checked_add(runtime_ms)
                .is_none_or(|end| end > item.deadline_ms)
            || envelope
                .pointer("/budget/max_seconds")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|seconds| seconds > runtime_ms / 1000)
            || envelope
                .pointer("/budget/max_turns")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|turns| turns > u64::from(plan.invocation_budget))
        {
            return Err(
                "worker runtime/invocations exceed parent queue, lease or dispatch budget".into(),
            );
        }
        let fresh: Vec<kiln_core::fair_admission::EligibilityInput> =
            match flags.get("--admission-observations") {
                Some(path) => read_json(Path::new(path))?,
                None => Vec::new(),
            };
        implementer.admission_before_transport(
            &attempt.identity,
            attempt.fence,
            &fresh,
            clock()?,
        )?;
    }
    let recovery = implementer.begin_dispatch_transport(
        &attempt.identity,
        attempt.fence,
        clock()?,
        &reservation.plan,
    )?;
    let command = if recovery { "recover" } else { "run" };
    let output_dir = &profile.output_directory;
    if !output_dir.is_absolute() || !profile.state_directory.is_absolute() {
        return Err("worker state/output paths must be absolute".into());
    }
    let mut directories = std::fs::DirBuilder::new();
    directories.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directories.mode(0o700);
    }
    directories
        .create(output_dir)
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(output_dir)
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("worker transport observations require a private output directory".into());
        }
    }
    let frozen_request = output_dir.join(format!("request-{}.json", request_id.as_str()));
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o400);
        }
        match options.open(&frozen_request) {
            Ok(mut file) => file
                .write_all(&request_bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&frozen_request).map_err(|error| error.to_string())?
                    != request_bytes
                {
                    return Err("frozen request conflict".into());
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    let args: Vec<std::ffi::OsString> = vec![
        "implementation-attempt".into(),
        command.into(),
        "--request".into(),
        frozen_request.as_os_str().into(),
        "--state".into(),
        profile.state_directory.as_os_str().into(),
    ];
    let launch = RunRequest {
        command: CommandSpec {
            program: profile.lattice_program.clone(),
            args,
            current_dir: output_dir.clone(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: u64::from(plan.deadline_minutes) * 60 * 1000 + 10_000,
            stdin_max_bytes: 1,
            stdout_max_bytes: 4 * 1024 * 1024,
            stderr_max_bytes: 4 * 1024 * 1024,
            poll_interval_ms: 10,
            teardown_timeout_ms: 5_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|error| format!("{error:?}"))?,
    };
    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handlers = Vec::new();
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        handlers.push(
            signal_hook::flag::register(signal, cancellation.clone())
                .map_err(|error| error.to_string())?,
        );
    }
    let cancel_flag = cancellation.clone();
    let done = finished.clone();
    let cancel_program = profile.lattice_program.clone();
    let cancel_state = profile.state_directory.clone();
    let cancel_root = output_dir.clone();
    let cancel_request = frozen_request.clone();
    let watcher = std::thread::spawn(move || {
        while !done.load(std::sync::atomic::Ordering::SeqCst) {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                let cancel_args: Vec<std::ffi::OsString> = vec![
                    "implementation-attempt".into(),
                    "cancel".into(),
                    "--request".into(),
                    cancel_request.as_os_str().into(),
                    "--state".into(),
                    cancel_state.as_os_str().into(),
                ];
                if let Ok(outcome_policy) = OutcomePolicy::new(vec![0], true, true) {
                    let result = bounded_exec::run(RunRequest {
                        command: CommandSpec {
                            program: cancel_program.clone(),
                            args: cancel_args,
                            current_dir: cancel_root.clone(),
                            environment_mode: EnvironmentMode::Clear,
                            environment: Vec::new(),
                            input: Input::Null,
                        },
                        limits: ExecutionLimits {
                            timeout_ms: 5_000,
                            stdin_max_bytes: 1,
                            stdout_max_bytes: 64 * 1024,
                            stderr_max_bytes: 64 * 1024,
                            poll_interval_ms: 10,
                            teardown_timeout_ms: 5_000,
                        },
                        termination_scope: TerminationScope::ProcessGroup,
                        outcome_policy,
                    });
                    if result.is_ok_and(|output| output.disposition == Disposition::Succeeded) {
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    let outcome = bounded_exec::run(launch);
    finished.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = watcher.join();
    for handler in handlers {
        signal_hook::low_level::unregister(handler);
    }
    let outcome = outcome.map_err(|_| {
        "Lattice outcome unknown after possible dispatch; retain state and recover".to_string()
    })?;
    // Raw transport observations remain separate from accepted candidate evidence.
    let prefix = format!(
        "{}-{}-{}",
        attempt.identity.as_str(),
        attempt.fence,
        if recovery { "recovery" } else { "dispatch" }
    );
    for (suffix, bytes) in [
        ("stdout", outcome.stdout.bytes.as_slice()),
        ("stderr", outcome.stderr.bytes.as_slice()),
    ] {
        use std::io::Write;
        let path = output_dir.join(format!(
            "{prefix}-{}.{}",
            blake3::hash(bytes).to_hex(),
            suffix
        ));
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&path).map_err(|error| error.to_string())? != bytes {
                    return Err("retained transport observation conflicts".into());
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    if outcome.disposition != Disposition::Succeeded {
        return Err("worker has no complete observed result; retain state and reconcile".into());
    }
    let observed: serde_json::Value = serde_json::from_slice(&outcome.stdout.bytes)
        .map_err(|_| "worker response malformed after possible dispatch")?;
    if observed.get("envelope") != request.get("envelope")
        || observed.get("schema").and_then(serde_json::Value::as_str)
            != Some("lattice.implementation-attempt-observation.v1")
    {
        return Err("worker output substituted the admitted envelope".into());
    }
    let workflow = ExternalRef::new("lattice", "implementation-attempt", request_id.as_str())
        .map_err(|error| format!("{error:?}"))?;
    implementer.reconcile_dispatch(&attempt.identity, attempt.fence, clock()?, &workflow)?;
    implementer.admission_execution_observed(&attempt.identity, attempt.fence, clock()?)?;
    // Observation failure cannot turn a valid native result into a failed
    // dispatch or grant candidate authority. Raw transport evidence is retained
    // above; absent/malformed telemetry is explicit in this diagnostic.
    let metrics_result = read_metrics_artifact(&profile.state_directory, &attempt, &observed)
        .and_then(|bytes| metrics::normalize(&request, &observed, &bytes))
        .and_then(|observation| {
            implementer.record_worker_observation(&attempt.identity, attempt.fence, &observation)
        });
    if let Err(error) = metrics_result {
        eprintln!("kiln worker metrics unavailable: {error}");
    }
    if cancellation.load(std::sync::atomic::Ordering::SeqCst)
        || observed
            .pointer("/result/disposition")
            .and_then(serde_json::Value::as_str)
            == Some("cancelled")
    {
        implementer.cancel(
            &attempt.identity,
            attempt.fence,
            clock()?,
            "provider cancellation observed; retained usage is not refunded",
        )?;
    }
    serde_json::to_string(&observed).map_err(|error| error.to_string())
}

#[cfg(test)]
mod transport_profile_tests {
    use super::transport_profile_mode_matches;
    use kiln_core::types::ContentId;

    #[test]
    fn profile_generation_and_native_binding_must_be_paired() {
        let native = ContentId::from_blake3_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v1",
            None
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v1",
            Some(&native)
        ));
        assert!(transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v2",
            Some(&native)
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v2",
            None
        ));
        assert!(!transport_profile_mode_matches(
            "kiln.lattice-implementation-cli.v3",
            Some(&native)
        ));
    }
}
