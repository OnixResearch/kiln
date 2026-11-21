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

#[path = "worker_binding.rs"]
mod binding;
#[path = "worker_metrics.rs"]
mod metrics;
#[path = "worker_settlement.rs"]
mod settlement;

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
/// One Lattice CLI command with a cleared ambient environment plus exactly the
/// supplied entries.
fn lattice_command(
    program: PathBuf,
    args: Vec<std::ffi::OsString>,
    current_dir: PathBuf,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> CommandSpec {
    CommandSpec {
        program,
        args,
        current_dir,
        environment_mode: EnvironmentMode::Clear,
        environment,
        input: Input::Null,
    }
}

/// Prepare every launch file before any owner grant is consumed: the private
/// output directory, the frozen request and, for the first launch, the bound
/// state root and its durable binding. Returns the frozen request and binding
/// paths. Each step is idempotent, and a conflicting frozen request or binding
/// is refused rather than replaced.
// r[impl kiln.implementation.lattice_cli.scenario.bound_root]
fn prepare_launch_files(
    profile: &WorkerTransportProfile,
    attempt: &ContentId,
    fence: u64,
    request_id: &ContentId,
    request_bytes: &[u8],
    first_launch: bool,
) -> Result<(PathBuf, PathBuf), String> {
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
                .write_all(request_bytes)
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
    // The first launch's program and state root are bound before its transport
    // intent; recovery never follows a later profile into another root.
    let binding_path = binding::TransportBinding::path(output_dir, attempt, fence);
    if first_launch {
        // The bound root exists from the first launch on, so recovery after a
        // loss before Lattice started still runs in it, never in a fresh root.
        directories
            .create(&profile.state_directory)
            .map_err(|error| error.to_string())?;
        binding::retain(
            &binding_path,
            &binding::TransportBinding::new(
                attempt,
                fence,
                &profile.lattice_program,
                &profile.executable_blake3,
                &profile.state_directory,
            ),
        )?;
    }
    Ok((frozen_request, binding_path))
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
// r[impl kiln.implementation.lattice_cli]
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
    // The admitted request alone names the one variable that may reach Lattice.
    let credential =
        settlement::credential_variable(&request).map_err(|error| error.to_string())?;
    let mut implementer = Implementer::open(&required(flags, "--database")?, enrollment)?;
    let attempt = implementer.authorize_current(&plan.attempt_identity, plan.fence, clock()?)?;
    // Before any durable step (plan reservation, request seal, admission or
    // transport intent), a fresh run needs its one credential variable.
    let recovery = implementer.dispatch_requires_recovery(&attempt.identity)?;
    let mut environment = if recovery {
        Vec::new()
    } else {
        settlement::credential_environment(credential, credential.and_then(std::env::var_os))
            .map_err(|error| error.to_string())?
    };
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
    // The exact request bytes must bind the plan Kiln reserves below.
    let plan_identity = kiln_core::implementation::admit_dispatch_plan(work, &attempt, &plan)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if !settlement::request_binds_plan(&request, plan_identity.as_str()) {
        return Err(
            "worker request does not bind the reserved dispatch plan; nothing was dispatched"
                .into(),
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
    if request_binding.plan != reservation.plan
        || reservation.plan != plan_identity
        || request_binding.request_digest != request_id
    {
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
    let fresh: Vec<kiln_core::fair_admission::EligibilityInput> = if recovery {
        Vec::new()
    } else {
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
        match flags.get("--admission-observations") {
            Some(path) => read_json(Path::new(path))?,
            None => Vec::new(),
        }
    };
    // Every fallible filesystem step of the first launch (output directory,
    // frozen request, bound state root and binding) precedes the owner-grant
    // gate, which moves committed owner reservations to unknown. A failure here
    // leaves the grants committed, so a retry after fixing the cause proceeds.
    let (frozen_request, binding_path) = prepare_launch_files(
        &profile,
        &attempt.identity,
        attempt.fence,
        &request_id,
        &request_bytes,
        !recovery,
    )?;
    let output_dir = &profile.output_directory;
    if !recovery {
        // Open item (predates this change): a crash between this gate and
        // `begin_dispatch_transport` leaves unknown grants with no transport
        // intent; neither a fresh run nor recovery can then proceed.
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
    let (lattice_program, state_directory) = if recovery {
        // Recovery never starts a worker, so it never receives the credential.
        environment.clear();
        let retained = binding::load(&binding_path)?;
        let state_is_directory = retained
            .as_ref()
            .is_some_and(|bound| binding::is_directory(&bound.state_directory));
        let bound = binding::recovery_launch(
            retained,
            &attempt.identity,
            attempt.fence,
            state_is_directory,
        )?;
        let bound_blake3 = ContentId::from_blake3_hex(bound.executable_blake3.clone())
            .map_err(|error| format!("{error:?}"))?;
        if program_identity(&bound.lattice_program)? != bound_blake3 {
            return Err("the bound Lattice executable changed; Lattice was not invoked".into());
        }
        (bound.lattice_program, bound.state_directory)
    } else {
        (
            profile.lattice_program.clone(),
            profile.state_directory.clone(),
        )
    };
    let launch_kind = if recovery {
        settlement::Launch::Recover
    } else {
        settlement::Launch::Run
    };
    let command = if recovery { "recover" } else { "run" };
    let args: Vec<std::ffi::OsString> = vec![
        "implementation-attempt".into(),
        command.into(),
        "--request".into(),
        frozen_request.as_os_str().into(),
        "--state".into(),
        state_directory.as_os_str().into(),
    ];
    let launch = RunRequest {
        command: lattice_command(
            lattice_program.clone(),
            args,
            output_dir.clone(),
            environment,
        ),
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
    let cancel_program = lattice_program.clone();
    let cancel_state = state_directory.clone();
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
                        // Cancellation starts no worker and never receives the credential.
                        command: lattice_command(
                            cancel_program.clone(),
                            cancel_args,
                            cancel_root.clone(),
                            Vec::new(),
                        ),
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
    if outcome.disposition == Disposition::ExitFailed {
        // Only Lattice can witness that no worker effect of this attempt began,
        // and only the first `run` speaks for the attempt's only launch.
        if let settlement::RefusalOutcome::BeforeEffect { reason } =
            settlement::classify_refusal(envelope, &outcome.stdout.bytes, launch_kind)
        {
            implementer.settle_dispatch_without_execution(
                &attempt.identity,
                attempt.fence,
                clock()?,
                &format!("lattice_refused_before_effect:{reason}"),
            )?;
            return Err(format!(
                "Lattice refused the attempt before any effect ({reason}); the attempt is settled without execution"
            ));
        }
    }
    if outcome.disposition != Disposition::Succeeded {
        return Err("worker has no complete observed result; retain state and reconcile".into());
    }
    let observed: serde_json::Value = serde_json::from_slice(&outcome.stdout.bytes)
        .map_err(|_| "worker response malformed after possible dispatch")?;
    if settlement::classify_observation(&request, &outcome.stdout.bytes, reservation.plan.as_str())?
        == settlement::Observed::ClosedBeforeExecution
    {
        if launch_kind != settlement::Launch::Recover {
            return Err(
                "closure reported outside bound recovery; retain state and reconcile".into(),
            );
        }
        // Recovery in the bound root exclusively closed the attempt: it never executes.
        implementer.settle_dispatch_without_execution(
            &attempt.identity,
            attempt.fence,
            clock()?,
            "lattice_closed_before_execution",
        )?;
        return serde_json::to_string(&observed).map_err(|error| error.to_string());
    }
    let workflow = ExternalRef::new("lattice", "implementation-attempt", request_id.as_str())
        .map_err(|error| format!("{error:?}"))?;
    implementer.reconcile_dispatch(&attempt.identity, attempt.fence, clock()?, &workflow)?;
    implementer.admission_execution_observed(&attempt.identity, attempt.fence, clock()?)?;
    // Observation failure cannot turn a valid native result into a failed
    // dispatch or grant candidate authority. Raw transport evidence is retained
    // above; absent/malformed telemetry is explicit in this diagnostic.
    let metrics_result = read_metrics_artifact(&state_directory, &attempt, &observed)
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

#[cfg(test)]
mod launch_environment_tests {
    use super::{lattice_command, settlement};
    use bounded_exec::{Disposition, ExecutionLimits, OutcomePolicy, RunRequest, TerminationScope};
    use std::ffi::OsString;

    /// What a real child launched like the Lattice process can read.
    fn observe(environment: Vec<(OsString, OsString)>) -> String {
        let probe = "printf '%s|%s|%s' \"${KILN_TEST_MODEL_TOKEN-unset}\" \"${HOME-unset}\" \"${CARGO_MANIFEST_DIR-unset}\"";
        let outcome = bounded_exec::run(RunRequest {
            command: lattice_command(
                "/bin/sh".into(),
                vec!["-c".into(), probe.into()],
                std::env::temp_dir(),
                environment,
            ),
            limits: ExecutionLimits {
                timeout_ms: 10_000,
                stdin_max_bytes: 1,
                stdout_max_bytes: 4096,
                stderr_max_bytes: 4096,
                poll_interval_ms: 10,
                teardown_timeout_ms: 5_000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true)
                .unwrap_or_else(|_| unreachable!()),
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(outcome.disposition, Disposition::Succeeded);
        String::from_utf8(outcome.stdout.bytes).unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.implementation.lattice_cli.scenario.credential]
    #[test]
    fn only_the_named_credential_reaches_the_lattice_process() {
        let request = serde_json::json!({"omp": {"model_broker": {
            "credential": {"provider": "env", "key": "KILN_TEST_MODEL_TOKEN"}
        }}});
        let name = settlement::credential_variable(&request).unwrap_or_else(|_| unreachable!());
        let environment =
            settlement::credential_environment(name, Some(OsString::from("fake-value")))
                .unwrap_or_else(|_| unreachable!());
        // The named value is delivered; ambient HOME and Cargo variables are withheld.
        assert_eq!(observe(environment), "fake-value|unset|unset");
        // No declared credential keeps the fully cleared environment.
        let undeclared = serde_json::json!({"omp": {"model_broker": {"credential": null}}});
        let none = settlement::credential_variable(&undeclared).unwrap_or_else(|_| unreachable!());
        let cleared = settlement::credential_environment(none, Some(OsString::from("fake-value")))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(observe(cleared), "unset|unset|unset");
    }
}

#[cfg(test)]
mod credential_ordering_tests {
    use super::run;
    use kiln_core::implementation::{
        AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace,
        Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationDispatchPlan, ImplementationWork,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_implementer::Implementer;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    const LEASE_MS: u64 = 600_000;
    /// A variable no test environment defines.
    const UNSET: &str = "KILN_TEST_UNSET_LATTICE_CREDENTIAL_45";

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn enrollment() -> Enrollment {
        Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        }
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: Some(
                ExternalRef::new("git", "commit", &"a".repeat(40))
                    .unwrap_or_else(|_| unreachable!()),
            ),
            title: "Credential ordering".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#45")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#45".to_string(),
                version: content('1'),
            }],
        }
    }

    fn write(path: &Path, value: &impl serde::Serialize) {
        std::fs::write(
            path,
            serde_json::to_vec(value).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
    }

    /// Run the transport for one claimed attempt and report its result and
    /// whether any dispatch plan was reserved.
    fn dispatch(credential: serde_json::Value) -> (Result<String, String>, bool) {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let root = directory.path();
        let database = root.join("kiln.sqlite");
        let now = super::clock().unwrap_or_else(|_| unreachable!());
        let work = work();
        let attempt = {
            let mut owner = Implementer::open(&database, enrollment())
                .unwrap_or_else(|error| unreachable!("{error}"));
            owner
                .admit(&work, None)
                .unwrap_or_else(|error| unreachable!("{error}"));
            owner
                .claim_next(now, LEASE_MS)
                .unwrap_or_else(|error| unreachable!("{error}"))
                .unwrap_or_else(|| unreachable!())
        };
        let plan = ImplementationDispatchPlan {
            schema: DISPATCH_PLAN_SCHEMA.into(),
            work_identity: attempt.work_identity.clone(),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            workspace: DispatchWorkspace {
                identity: content('e'),
                isolation: WorkspaceIsolation::IsolatedSnapshot,
                network: false,
                credentials: false,
                projection: None,
            },
            provider_profile: ProviderProfileId::new("lattice-implementation")
                .unwrap_or_else(|_| unreachable!()),
            permitted_paths: work.permitted_paths.clone(),
            permitted_effects: work.permitted_effects.clone(),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 1,
            token_budget: 1000,
            invocation_budget: 1,
            expected_output: content('f'),
            execution: None,
        };
        write(&root.join("plan.json"), &plan);
        write(&root.join("enrollment.json"), &enrollment());
        write(
            &root.join("request.json"),
            &serde_json::json!({"envelope": {}, "omp": {"model_broker": {"credential": credential}}}),
        );
        write(
            &root.join("transport.json"),
            &serde_json::json!({
                "schema": "kiln.lattice-implementation-cli.v2",
                "lattice_program": root.join("not-a-store-program"),
                "executable_blake3": "b".repeat(64),
                "state_directory": root.join("state"),
                "output_directory": root.join("transport"),
            }),
        );
        let flags = BTreeMap::from([
            ("--database".to_string(), database.display().to_string()),
            (
                "--enrollment".to_string(),
                root.join("enrollment.json").display().to_string(),
            ),
            (
                "--dispatch-plan".to_string(),
                root.join("plan.json").display().to_string(),
            ),
            (
                "--worker-request".to_string(),
                root.join("request.json").display().to_string(),
            ),
            (
                "--worker-transport-profile".to_string(),
                root.join("transport.json").display().to_string(),
            ),
        ]);
        let result = run(&flags);
        let reserved = Implementer::open(&database, enrollment())
            .and_then(|mut owner| owner.dispatch_reservation(&attempt.identity))
            .unwrap_or_else(|error| unreachable!("{error}"))
            .is_some();
        (result, reserved)
    }

    // r[verify kiln.implementation.lattice_cli.scenario.credential]
    #[test]
    fn a_refused_credential_is_rejected_before_any_plan_reservation() {
        let (missing, reserved) = dispatch(serde_json::json!({"provider": "env", "key": UNSET}));
        assert!(
            missing
                .as_ref()
                .is_err_and(|error| error.starts_with("worker_credential_missing:")),
            "{missing:?}"
        );
        assert!(!reserved);
        let (reserved_name, reserved) =
            dispatch(serde_json::json!({"provider": "env", "key": "PATH"}));
        assert!(
            reserved_name
                .as_ref()
                .is_err_and(|error| error.starts_with("worker_credential_reserved_name:")),
            "{reserved_name:?}"
        );
        assert!(!reserved);
        // Without a declared credential the credential stage passes, and the
        // next check (the immutable executable) refuses, still before any
        // reservation.
        let (undeclared, reserved) = dispatch(serde_json::Value::Null);
        assert!(
            undeclared
                .as_ref()
                .is_err_and(|error| error.contains("not an admitted immutable Nix program")),
            "{undeclared:?}"
        );
        assert!(!reserved);
    }
}

#[cfg(test)]
#[path = "worker_preparation_tests.rs"]
mod preparation_tests;
