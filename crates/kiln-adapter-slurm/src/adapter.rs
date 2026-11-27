use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, ExecutionOutput, Input,
    Operation as ExecOperation, OutcomePolicy, RunError, RunRequest, TerminationScope, run,
};
use kiln_core::domain::Outcome;
use kiln_core::remote::{RemoteAction, RemoteCapability, RemoteObservation, RemoteProfile};
use kiln_core::types::{Diagnostic, EffectId, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, RemoteExecutionPort, RemoteLifecyclePort,
    RemoteTransportObservation,
};

use crate::protocol::{
    SLURM_JOB_NATIVE_KIND, SLURM_PROVIDER_NAMESPACE, SlurmCorrelation, SlurmJobPhase, SlurmProfile,
    SlurmSubmissionSpec, admit_profile, admit_submission, build_sbatch_args, build_scancel_args,
    build_squeue_args, parse_sbatch_output, parse_squeue_output, validate_slurm_operation_ref,
};

const SUCCESS_EXIT_CODE: i32 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
enum SubmissionState {
    NotSubmitted,
    Submitted {
        operation: ExternalRef,
        last_observation: Box<RemoteObservation>,
    },
    Ambiguous,
    Failed {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StagedSubmission {
    action: RemoteAction,
    spec: SlurmSubmissionSpec,
    correlation: SlurmCorrelation,
    uploaded: bool,
    submission: SubmissionState,
}

#[derive(Debug)]
pub struct SlurmAdapter {
    profile: SlurmProfile,
    staged: BTreeMap<EffectId, StagedSubmission>,
    operations: BTreeMap<ExternalRef, EffectId>,
}

impl SlurmAdapter {
    /// Constructs a new `SlurmAdapter`, failing closed if the profile is disabled or invalid.
    pub fn new(profile: SlurmProfile) -> PortResult<Self> {
        admit_profile(&profile).map_err(PortFailure::Domain)?;
        Ok(Self {
            profile,
            staged: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &SlurmProfile {
        &self.profile
    }

    pub fn remote_profile(&self) -> PortResult<RemoteProfile> {
        self.profile.to_remote_profile().map_err(PortFailure::Domain)
    }

    /// Validates and stages a concrete Slurm submission for a `RemoteAction`.
    ///
    /// Pure admission runs before any external process invocation. Repeating the
    /// exact same `(action, spec)` for an `effect_id` is idempotent; conflicting
    /// payloads fail closed.
    pub fn stage_action(
        &mut self,
        action: &RemoteAction,
        spec: SlurmSubmissionSpec,
    ) -> PortResult<SlurmCorrelation> {
        let correlation =
            admit_submission(&self.profile, action, &spec).map_err(PortFailure::Domain)?;
        if let Some(existing) = self.staged.get(&action.effect_id) {
            if existing.action == *action && existing.spec == spec {
                return Ok(existing.correlation.clone());
            }
            return Err(domain(
                "slurm_replayed_action_conflict",
                "slurm.submission",
                "replayed effect_id conflicts with previously staged RemoteAction or spec",
            ));
        }
        let active_count = self
            .staged
            .values()
            .filter(|entry| {
                match &entry.submission {
                    SubmissionState::Ambiguous => true,
                    SubmissionState::Submitted {
                        last_observation, ..
                    } => matches!(last_observation.outcome, Outcome::Pending | Outcome::Unknown),
                    SubmissionState::NotSubmitted | SubmissionState::Failed { .. } => false,
                }
            })
            .count();
        if active_count >= self.profile.bounds.max_in_flight {
            return Err(PortFailure::Retryable(infrastructure(
                "slurm_in_flight_bound",
                "Slurm adapter reached its maximum in-flight job bound",
            )));
        }
        self.staged.insert(
            action.effect_id.clone(),
            StagedSubmission {
                action: action.clone(),
                spec,
                correlation: correlation.clone(),
                uploaded: false,
                submission: SubmissionState::NotSubmitted,
            },
        );
        Ok(correlation)
    }

    /// Restores a previously submitted Slurm job into the adapter state for reconciliation.
    pub fn restore_operation(
        &mut self,
        action: &RemoteAction,
        spec: SlurmSubmissionSpec,
        operation: &ExternalRef,
        last_outcome: Outcome,
    ) -> PortResult<()> {
        let correlation =
            admit_submission(&self.profile, action, &spec).map_err(PortFailure::Domain)?;
        let _job_id = validate_slurm_operation_ref(operation).map_err(PortFailure::Domain)?;
        if let Some(existing_effect) = self.operations.get(operation)
            && existing_effect != &action.effect_id
        {
            return Err(domain(
                "slurm_operation_conflict",
                "slurm.operation",
                "Slurm operation is already bound to a different effect_id",
            ));
        }
        let observation = RemoteObservation {
            action: correlation.action_ref.clone(),
            operation: Some(operation.clone()),
            cache_result: None,
            execution: Some(operation.clone()),
            worker: None,
            logs: Vec::new(),
            outcome: last_outcome,
            required_outputs_present: last_outcome == Outcome::Success
                && action.declared_outputs.is_empty(),
        };
        if let Some(existing) = self.staged.get(&action.effect_id) {
            let same_operation = matches!(
                &existing.submission,
                SubmissionState::Submitted {
                    operation: existing_op,
                    ..
                } if existing_op == operation
            );
            if existing.action != *action || existing.spec != spec || !same_operation {
                return Err(domain(
                    "slurm_restore_conflict",
                    "slurm.restore",
                    "restored submission conflicts with existing staged record",
                ));
            }
            return Ok(());
        }
        self.operations
            .insert(operation.clone(), action.effect_id.clone());
        self.staged.insert(
            action.effect_id.clone(),
            StagedSubmission {
                action: action.clone(),
                spec,
                correlation,
                uploaded: true,
                submission: SubmissionState::Submitted {
                    operation: operation.clone(),
                    last_observation: Box::new(observation),
                },
            },
        );
        Ok(())
    }

    /// Returns the last tracked observation for an `effect_id`, if any.
    #[must_use]
    pub fn observation_for_effect(&self, effect_id: &EffectId) -> Option<&RemoteObservation> {
        match self.staged.get(effect_id).map(|entry| &entry.submission) {
            Some(SubmissionState::Submitted {
                last_observation, ..
            }) => Some(last_observation.as_ref()),
            _ => None,
        }
    }

    /// Returns the Slurm job phase classification by invoking `squeue` for a tracked operation.
    pub fn poll_phase(&mut self, operation: &ExternalRef) -> PortResult<(SlurmJobPhase, RemoteTransportObservation)> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::Poll)?;
        let job_id = validate_slurm_operation_ref(operation)
            .map_err(PortFailure::Domain)?
            .to_string();
        let effect_id = self
            .operations
            .get(operation)
            .cloned()
            .ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.operation",
                    "operation reference does not match any known Slurm submission",
                )
            })?;
        let action_ref = self
            .staged
            .get(&effect_id)
            .ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.operation",
                    "tracked Slurm submission is missing",
                )
            })?
            .correlation
            .action_ref
            .clone();

        let args = build_squeue_args(&job_id);
        let exec_result = run_bounded_cli(
            &self.profile.executables.squeue_path,
            &args,
            Path::new("/"),
            self.profile.bounds.execution_limits(),
        );
        match classify_cli_result(exec_result, "slurm_squeue")? {
            CliOutcome::ExitedZero(stdout_bytes) => {
                let stdout = std::str::from_utf8(&stdout_bytes).map_err(|_| {
                    domain(
                        "slurm_malformed_squeue_output",
                        "squeue.stdout",
                        "squeue output is not valid UTF-8",
                    )
                })?;
                let record =
                    parse_squeue_output(&job_id, stdout).map_err(PortFailure::Domain)?;
                let outcome = record.phase.to_kiln_outcome();
                let entry = self.staged.get_mut(&effect_id).ok_or_else(|| {
                    domain(
                        "slurm_unknown_operation",
                        "slurm.operation",
                        "tracked Slurm submission is missing",
                    )
                })?;
                let required_outputs_present = if outcome == Outcome::Success {
                    check_declared_outputs(&entry.action, &entry.spec)?
                } else {
                    false
                };
                let logs = collected_log_refs(operation, &entry.spec)?;
                let observation = RemoteObservation {
                    action: action_ref,
                    operation: Some(operation.clone()),
                    cache_result: None,
                    execution: Some(operation.clone()),
                    worker: record
                        .node_list
                        .as_deref()
                        .map(|nodes| ExternalRef::new(SLURM_PROVIDER_NAMESPACE, "node", nodes))
                        .transpose()
                        .map_err(PortFailure::Domain)?,
                    logs,
                    outcome,
                    required_outputs_present,
                };
                entry.submission = SubmissionState::Submitted {
                    operation: operation.clone(),
                    last_observation: Box::new(observation.clone()),
                };
                Ok((
                    record.phase,
                    RemoteTransportObservation::Observation(Box::new(observation)),
                ))
            }
            CliOutcome::ExitedNonZero { code, stderr } => Err(PortFailure::Infrastructure(
                infrastructure(
                    "slurm_squeue_failed",
                    &format!("squeue exited with status {code:?}: {stderr}"),
                ),
            )),
            CliOutcome::Ambiguous => Ok((
                SlurmJobPhase::Unknown { raw_state: None },
                RemoteTransportObservation::DisconnectedAfterAcceptance {
                    action: action_ref,
                    operation: Some(operation.clone()),
                },
            )),
        }
    }

    fn require_capability(&self, capability: RemoteCapability) -> PortResult<()> {
        if !self.profile.capabilities.contains(&capability) {
            return Err(domain(
                "remote_capability_unsupported",
                "remote.capabilities",
                "Slurm profile does not declare the requested capability",
            ));
        }
        Ok(())
    }
}

impl RemoteExecutionPort for SlurmAdapter {
    fn upload(&mut self, action: &RemoteAction) -> PortResult<()> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::CasUpload)?;
        if action.provider != self.profile.provider {
            return Err(domain(
                "slurm_provider_mismatch",
                "remote_action.provider",
                "remote action provider does not match the selected Slurm profile",
            ));
        }
        let entry = self.staged.get_mut(&action.effect_id).ok_or_else(|| {
            domain(
                "slurm_unregistered_action",
                "slurm.upload",
                "action must be staged with an admitted Slurm submission spec before upload",
            )
        })?;
        if entry.action != *action {
            return Err(domain(
                "slurm_action_mismatch",
                "slurm.upload",
                "upload RemoteAction does not match the staged submission for this effect_id",
            ));
        }
        entry.uploaded = true;
        Ok(())
    }

    fn execute(&mut self, action: &RemoteAction) -> PortResult<RemoteTransportObservation> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::Execute)?;
        if action.provider != self.profile.provider {
            return Err(domain(
                "slurm_provider_mismatch",
                "remote_action.provider",
                "remote action provider does not match the selected Slurm profile",
            ));
        }
        let entry = self.staged.get(&action.effect_id).ok_or_else(|| {
            domain(
                "slurm_unregistered_action",
                "slurm.execute",
                "action must be staged with an admitted Slurm submission spec before execute",
            )
        })?;
        if entry.action != *action {
            return Err(domain(
                "slurm_action_mismatch",
                "slurm.execute",
                "execute RemoteAction does not match the staged submission for this effect_id",
            ));
        }
        match &entry.submission {
            SubmissionState::Submitted {
                last_observation, ..
            } => {
                return Ok(RemoteTransportObservation::Observation(
                    last_observation.clone(),
                ));
            }
            SubmissionState::Ambiguous => {
                return Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                    action: entry.correlation.action_ref.clone(),
                    operation: None,
                });
            }
            SubmissionState::Failed { code, message } => {
                return Err(PortFailure::Infrastructure(infrastructure(code, message)));
            }
            SubmissionState::NotSubmitted => {}
        }
        if self.profile.capabilities.contains(&RemoteCapability::CasUpload) && !entry.uploaded {
            return Err(domain(
                "slurm_upload_required",
                "slurm.execute",
                "action must be uploaded before execute when CasUpload is enabled",
            ));
        }

        let action_ref = entry.correlation.action_ref.clone();
        let working_dir = entry.spec.working_dir.clone();
        let args = build_sbatch_args(&entry.spec, &entry.correlation).map_err(PortFailure::Domain)?;
        let limits = self.profile.bounds.execution_limits();
        let exec_result = run_bounded_cli(
            &self.profile.executables.sbatch_path,
            &args,
            &working_dir,
            limits,
        );
        let outcome = match classify_cli_result(exec_result, "slurm_sbatch") {
            Ok(outcome) => outcome,
            Err(err @ PortFailure::TimedOut(_)) => {
                if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                    entry.submission = SubmissionState::Ambiguous;
                }
                return Err(err);
            }
            Err(err) => return Err(err),
        };
        match outcome {
            CliOutcome::ExitedZero(stdout_bytes) => {
                let stdout = std::str::from_utf8(&stdout_bytes).map_err(|_| {
                    if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                        entry.submission = SubmissionState::Ambiguous;
                    }
                    domain(
                        "slurm_malformed_sbatch_output",
                        "sbatch.stdout",
                        "sbatch output is not valid UTF-8",
                    )
                })?;
                let parsed = match parse_sbatch_output(
                    stdout,
                    self.profile.cluster_name.as_deref(),
                ) {
                    Ok(parsed) => parsed,
                    Err(diag) => {
                        if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                            entry.submission = SubmissionState::Ambiguous;
                        }
                        return Err(PortFailure::Domain(diag));
                    }
                };
                let operation = ExternalRef::new(
                    SLURM_PROVIDER_NAMESPACE,
                    SLURM_JOB_NATIVE_KIND,
                    &parsed.job_id,
                )
                .map_err(PortFailure::Domain)?;
                if let Some(existing_effect) = self.operations.get(&operation)
                    && existing_effect != &action.effect_id
                {
                    if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                        entry.submission = SubmissionState::Ambiguous;
                    }
                    return Err(domain(
                        "slurm_duplicate_job_id",
                        "sbatch.stdout",
                        "sbatch returned a job ID already bound to another effect_id",
                    ));
                }
                let entry = self.staged.get_mut(&action.effect_id).ok_or_else(|| {
                    domain(
                        "slurm_unregistered_action",
                        "slurm.execute",
                        "staged submission vanished during execute",
                    )
                })?;
                let logs = collected_log_refs(&operation, &entry.spec)?;
                let observation = RemoteObservation {
                    action: action_ref,
                    operation: Some(operation.clone()),
                    cache_result: None,
                    execution: Some(operation.clone()),
                    worker: parsed
                        .cluster
                        .as_deref()
                        .map(|c| ExternalRef::new(SLURM_PROVIDER_NAMESPACE, "cluster", c))
                        .transpose()
                        .map_err(PortFailure::Domain)?,
                    logs,
                    outcome: Outcome::Pending,
                    required_outputs_present: false,
                };
                entry.submission = SubmissionState::Submitted {
                    operation: operation.clone(),
                    last_observation: Box::new(observation.clone()),
                };
                self.operations.insert(operation, action.effect_id.clone());
                Ok(RemoteTransportObservation::Observation(Box::new(
                    observation,
                )))
            }
            CliOutcome::ExitedNonZero { code, stderr } => {
                let code_str = "slurm_sbatch_failed".to_string();
                let message = format!("sbatch exited with status {code:?}: {stderr}");
                if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                    entry.submission = SubmissionState::Failed {
                        code: code_str.clone(),
                        message: message.clone(),
                    };
                }
                Err(PortFailure::Infrastructure(infrastructure(&code_str, &message)))
            }
            CliOutcome::Ambiguous => {
                if let Some(entry) = self.staged.get_mut(&action.effect_id) {
                    entry.submission = SubmissionState::Ambiguous;
                }
                Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                    action: action_ref,
                    operation: None,
                })
            }
        }
    }

    fn cancel(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::Cancel)?;
        let job_id = validate_slurm_operation_ref(operation)
            .map_err(PortFailure::Domain)?
            .to_string();
        let effect_id = self
            .operations
            .get(operation)
            .cloned()
            .ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.operation",
                    "cancel operation does not match any known Slurm submission",
                )
            })?;
        let action_ref = self
            .staged
            .get(&effect_id)
            .ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.operation",
                    "tracked Slurm submission is missing",
                )
            })?
            .correlation
            .action_ref
            .clone();

        let args = build_scancel_args(&job_id);
        let exec_result = run_bounded_cli(
            &self.profile.executables.scancel_path,
            &args,
            Path::new("/"),
            self.profile.bounds.execution_limits(),
        );
        match classify_cli_result(exec_result, "slurm_scancel")? {
            CliOutcome::ExitedZero(_) => {
                let entry = self.staged.get_mut(&effect_id).ok_or_else(|| {
                    domain(
                        "slurm_unknown_operation",
                        "slurm.operation",
                        "tracked Slurm submission is missing",
                    )
                })?;
                let logs = collected_log_refs(operation, &entry.spec)?;
                let observation = RemoteObservation {
                    action: action_ref,
                    operation: Some(operation.clone()),
                    cache_result: None,
                    execution: Some(operation.clone()),
                    worker: None,
                    logs,
                    outcome: Outcome::Cancelled,
                    required_outputs_present: false,
                };
                entry.submission = SubmissionState::Submitted {
                    operation: operation.clone(),
                    last_observation: Box::new(observation.clone()),
                };
                Ok(RemoteTransportObservation::Observation(Box::new(
                    observation,
                )))
            }
            CliOutcome::ExitedNonZero { code, stderr } => Err(PortFailure::Infrastructure(
                infrastructure(
                    "slurm_scancel_failed",
                    &format!("scancel exited with status {code:?}: {stderr}"),
                ),
            )),
            CliOutcome::Ambiguous => {
                Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                    action: action_ref,
                    operation: Some(operation.clone()),
                })
            }
        }
    }
}

impl RemoteLifecyclePort for SlurmAdapter {
    fn poll(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        let (_phase, observation) = self.poll_phase(operation)?;
        Ok(observation)
    }

    fn wait(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::Wait)?;
        let max_polls = self.profile.bounds.max_wait_polls;
        let interval = Duration::from_millis(self.profile.bounds.poll_interval_ms);
        for attempt in 0..max_polls {
            let (_phase, observation) = self.poll_phase(operation)?;
            match &observation {
                RemoteTransportObservation::Observation(obs)
                    if obs.outcome == Outcome::Pending =>
                {
                    if attempt.saturating_add(1) < max_polls {
                        thread::sleep(interval);
                    }
                }
                _ => return Ok(observation),
            }
        }
        Err(PortFailure::TimedOut(infrastructure(
            "slurm_wait_timeout",
            "Slurm wait exceeded max_wait_polls before reaching a non-pending state",
        )))
    }

    fn download(&mut self, output: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::Download)?;
        output.validate().map_err(PortFailure::Domain)?;
        if max_bytes == 0 || max_bytes > self.profile.bounds.max_log_bytes {
            return Err(domain(
                "slurm_download_bound",
                "slurm.download.max_bytes",
                "download byte bound must be positive and within max_log_bytes",
            ));
        }
        Err(domain(
            "remote_capability_unsupported",
            "slurm.download",
            "CAS output download is not supported by the Slurm batch adapter",
        ))
    }

    fn read_log(&mut self, log: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>> {
        admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        self.require_capability(RemoteCapability::LogStream)?;
        log.validate().map_err(PortFailure::Domain)?;
        if max_bytes == 0 || max_bytes > self.profile.bounds.max_log_bytes {
            return Err(domain(
                "slurm_log_bound",
                "slurm.read_log.max_bytes",
                "log read byte bound must be positive and within max_log_bytes",
            ));
        }
        if log.provider_namespace.as_str() != SLURM_PROVIDER_NAMESPACE {
            return Err(domain(
                "slurm_log_namespace",
                "slurm.read_log.namespace",
                "log reference must belong to the slurm namespace",
            ));
        }
        let path = resolve_log_path(&self.profile, &self.operations, &self.staged, log)?;
        read_bounded_file(&path, max_bytes)
    }
}

enum CliOutcome {
    ExitedZero(Vec<u8>),
    ExitedNonZero { code: Option<i32>, stderr: String },
    Ambiguous,
}

fn run_bounded_cli(
    program: &Path,
    args: &[String],
    current_dir: &Path,
    limits: ExecutionLimits,
) -> Result<ExecutionOutput, RunError> {
    let outcome_policy = OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true)
        .map_err(|_| RunError::InvalidLimits(bounded_exec::LimitError::TimeoutZero))?;
    let request = RunRequest {
        command: CommandSpec {
            program: program.to_path_buf(),
            args: args.iter().map(OsString::from).collect(),
            current_dir: current_dir.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits,
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy,
    };
    run(request)
}

fn classify_cli_result(
    result: Result<ExecutionOutput, RunError>,
    code_prefix: &str,
) -> PortResult<CliOutcome> {
    let output = match result {
        Ok(output) => output,
        Err(err) if execution_may_have_started(&err) => return Ok(CliOutcome::Ambiguous),
        Err(err) => {
            return Err(PortFailure::Infrastructure(infrastructure(
                &format!("{code_prefix}_spawn"),
                &format!("Slurm CLI invocation failed before execution: {err}"),
            )));
        }
    };
    match output.disposition {
        Disposition::Succeeded => Ok(CliOutcome::ExitedZero(output.stdout.bytes)),
        Disposition::ExitFailed => {
            let stderr = String::from_utf8_lossy(&output.stderr.bytes).trim().to_string();
            Ok(CliOutcome::ExitedNonZero {
                code: output.exit_code,
                stderr,
            })
        }
        Disposition::TimedOut => Err(PortFailure::TimedOut(infrastructure(
            &format!("{code_prefix}_timeout"),
            "Slurm CLI invocation exceeded its configured timeout",
        ))),
        Disposition::Cancelled => Err(PortFailure::Cancelled(infrastructure(
            &format!("{code_prefix}_cancelled"),
            "Slurm CLI invocation was cancelled",
        ))),
        Disposition::OutputLimitExceeded(_) => Ok(CliOutcome::Ambiguous),
    }
}

const fn execution_may_have_started(error: &RunError) -> bool {
    match error {
        RunError::InvalidLimits(_)
        | RunError::EmptyProgram
        | RunError::ProgramNotAbsolute
        | RunError::EmptyCurrentDirectory
        | RunError::CurrentDirectoryNotAbsolute
        | RunError::DuplicateEnvironmentName
        | RunError::InputLimitExceeded
        | RunError::Io {
            operation: ExecOperation::Spawn,
            ..
        } => false,
        RunError::MissingPipe(_)
        | RunError::Io { .. }
        | RunError::CaptureCountOverflow(_)
        | RunError::WorkerDisconnected(_)
        | RunError::WorkerPanicked(_)
        | RunError::WorkerTeardownTimedOut(_)
        | RunError::ChildTeardownTimedOut
        | RunError::ProcessIdentifierOverflow => true,
    }
}

fn check_declared_outputs(action: &RemoteAction, spec: &SlurmSubmissionSpec) -> PortResult<bool> {
    if action.declared_outputs.is_empty() {
        return Ok(true);
    }
    for output in &action.declared_outputs {
        if output.provider_namespace.as_str() == SLURM_PROVIDER_NAMESPACE
            && output.native_kind.as_str() == "relative-output"
        {
            let rel = Path::new(output.native_id.as_str());
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Ok(false);
            }
            if !spec.working_dir.join(rel).exists() {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

fn collected_log_refs(
    operation: &ExternalRef,
    spec: &SlurmSubmissionSpec,
) -> PortResult<Vec<ExternalRef>> {
    let job_id = operation.native_id.as_str();
    let mut logs = Vec::new();
    if spec.stdout_path.is_some() {
        logs.push(
            ExternalRef::new(SLURM_PROVIDER_NAMESPACE, "job-stdout", job_id)
                .map_err(PortFailure::Domain)?,
        );
    }
    if spec.stderr_path.is_some() {
        logs.push(
            ExternalRef::new(SLURM_PROVIDER_NAMESPACE, "job-stderr", job_id)
                .map_err(PortFailure::Domain)?,
        );
    }
    Ok(logs)
}

fn resolve_log_path(
    profile: &SlurmProfile,
    operations: &BTreeMap<ExternalRef, EffectId>,
    staged: &BTreeMap<EffectId, StagedSubmission>,
    log: &ExternalRef,
) -> PortResult<PathBuf> {
    let log_root = profile.log_root.as_ref().ok_or_else(|| {
        domain(
            "slurm_log_root_unconfigured",
            "slurm.log_root",
            "log_root must be configured before reading Slurm job logs",
        )
    })?;
    let candidate = match log.native_kind.as_str() {
        "job-stdout" | "job-stderr" => {
            let op_ref = ExternalRef::new(
                SLURM_PROVIDER_NAMESPACE,
                SLURM_JOB_NATIVE_KIND,
                log.native_id.as_str(),
            )
            .map_err(PortFailure::Domain)?;
            let effect_id = operations.get(&op_ref).ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.read_log",
                    "log reference names an unknown Slurm job",
                )
            })?;
            let entry = staged.get(effect_id).ok_or_else(|| {
                domain(
                    "slurm_unknown_operation",
                    "slurm.read_log",
                    "tracked Slurm submission is missing",
                )
            })?;
            let maybe_path = if log.native_kind.as_str() == "job-stdout" {
                entry.spec.stdout_path.as_ref()
            } else {
                entry.spec.stderr_path.as_ref()
            };
            maybe_path
                .cloned()
                .ok_or_else(|| {
                    domain(
                        "slurm_log_not_configured",
                        "slurm.read_log",
                        "requested Slurm job stream path was not configured on submission",
                    )
                })?
        }
        "log" => {
            let rel = Path::new(log.native_id.as_str());
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(domain(
                    "slurm_log_path_denied",
                    "slurm.read_log",
                    "relative log reference must not be absolute or traverse parents",
                ));
            }
            log_root.join(rel)
        }
        _ => {
            return Err(domain(
                "slurm_log_kind_unsupported",
                "slurm.read_log.native_kind",
                "unsupported Slurm log reference kind",
            ));
        }
    };
    let canonical_root = log_root.canonicalize().map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "slurm_log_root_unavailable",
            "configured Slurm log_root could not be canonicalized",
        ))
    })?;
    let canonical_file = candidate.canonicalize().map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "slurm_log_unavailable",
            "requested Slurm log file could not be opened",
        ))
    })?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(domain(
            "slurm_log_path_denied",
            "slurm.read_log",
            "resolved log path escapes configured Slurm log_root",
        ));
    }
    Ok(canonical_file)
}

fn read_bounded_file(path: &Path, max_bytes: usize) -> PortResult<Vec<u8>> {
    let file = File::open(path).map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "slurm_log_open",
            "Slurm log file could not be opened",
        ))
    })?;
    let metadata = file.metadata().map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "slurm_log_metadata",
            "Slurm log file metadata is unavailable",
        ))
    })?;
    if !metadata.is_file() {
        return Err(domain(
            "slurm_log_not_file",
            "slurm.read_log",
            "Slurm log target is not a regular file",
        ));
    }
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    let mut buffer = Vec::new();
    file.take(limit)
        .read_to_end(&mut buffer)
        .map_err(|_| {
            PortFailure::Infrastructure(infrastructure(
                "slurm_log_read",
                "Slurm log file could not be read",
            ))
        })?;
    Ok(buffer)
}

fn domain(code: &str, field: &'static str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}
