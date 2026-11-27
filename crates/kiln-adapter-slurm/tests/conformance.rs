use std::collections::BTreeSet;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use kiln_adapter_remote::{
    ObservationMode, cancel_operation, dispatch, observe_operation, read_bounded_log,
};
use kiln_adapter_slurm::{
    SLURM_PROFILE_SCHEMA, SlurmAdapter, SlurmExecutablePaths, SlurmJobPhase, SlurmPolicyBounds,
    SlurmProfile, SlurmRequestedFeature, SlurmResources, SlurmSubmissionSpec,
};
use kiln_core::domain::Outcome;
use kiln_core::provider::ProviderFailureDisposition;
use kiln_core::remote::{RemoteAction, RemoteCapability};
use kiln_core::types::{
    AttemptId, BLAKE3_HEX_BYTES, ContentId, EffectId, ExternalRef, PipelineRevisionId,
    ProviderProfileId,
};
use kiln_provider_api::{
    PortFailure, RemoteExecutionPort, RemoteLifecyclePort, RemoteTransportObservation,
};
use tempfile::TempDir;

struct MockSlurmCluster {
    dir: TempDir,
    work_dir: PathBuf,
    log_dir: PathBuf,
    sbatch_path: PathBuf,
    squeue_path: PathBuf,
    scancel_path: PathBuf,
    sbatch_count_file: PathBuf,
    sbatch_argv_file: PathBuf,
    squeue_invoked_file: PathBuf,
    scancel_invoked_file: PathBuf,
}

impl MockSlurmCluster {
    fn new() -> Self {
        let dir = TempDir::new().unwrap_or_else(|_| unreachable!());
        let root = dir.path();
        let bin_dir = root.join("bin");
        let work_dir = root.join("work");
        let log_dir = root.join("logs");
        fs::create_dir_all(&bin_dir).unwrap_or_else(|_| unreachable!());
        fs::create_dir_all(&work_dir).unwrap_or_else(|_| unreachable!());
        fs::create_dir_all(&log_dir).unwrap_or_else(|_| unreachable!());
        Self {
            sbatch_path: bin_dir.join("sbatch"),
            squeue_path: bin_dir.join("squeue"),
            scancel_path: bin_dir.join("scancel"),
            sbatch_count_file: root.join("sbatch_count"),
            sbatch_argv_file: root.join("sbatch_argv"),
            squeue_invoked_file: root.join("squeue_invoked"),
            scancel_invoked_file: root.join("scancel_invoked"),
            work_dir,
            log_dir,
            dir,
        }
    }

    fn write_script(&self, path: &Path, body: &str) {
        fs::write(path, body).unwrap_or_else(|_| unreachable!());
        fs::set_permissions(path, Permissions::from_mode(0o755))
            .unwrap_or_else(|_| unreachable!());
    }

    fn install_happy_scripts(&self) {
        let state_file = self.dir.path().join("squeue_poll_count");
        self.write_script(
            &self.sbatch_path,
            &format!(
                "#!/bin/sh\n\
                 count=0\n\
                 if [ -f \"{}\" ]; then read -r count < \"{}\" || count=0; fi\n\
                 count=$((count + 1))\n\
                 printf '%s\\n' \"$count\" > \"{}\"\n\
                 : > \"{}\"\n\
                 for a in \"$@\"; do printf '%s\\n' \"$a\" >> \"{}\"; done\n\
                 printf '9001;cluster-a\\n'\n",
                self.sbatch_count_file.display(),
                self.sbatch_count_file.display(),
                self.sbatch_count_file.display(),
                self.sbatch_argv_file.display(),
                self.sbatch_argv_file.display(),
            ),
        );
        self.write_script(
            &self.squeue_path,
            &format!(
                "#!/bin/sh\n\
                 : > \"{}\"\n\
                 count=0\n\
                 if [ -f \"{}\" ]; then read -r count < \"{}\" || count=0; fi\n\
                 count=$((count + 1))\n\
                 printf '%s\\n' \"$count\" > \"{}\"\n\
                 if [ \"$count\" -eq 1 ]; then\n\
                   printf '9001|PENDING||\\n'\n\
                 elif [ \"$count\" -eq 2 ]; then\n\
                   printf '9001|RUNNING||node-01|None\\n'\n\
                 else\n\
                   printf '9001|COMPLETED|0:0|node-01|None\\n'\n\
                 fi\n",
                self.squeue_invoked_file.display(),
                state_file.display(),
                state_file.display(),
                state_file.display(),
            ),
        );
        self.write_script(
            &self.scancel_path,
            &format!(
                "#!/bin/sh\n\
                 printf '%s\\n' \"$*\" > \"{}\"\n",
                self.scancel_invoked_file.display(),
            ),
        );
    }

    fn profile(&self) -> SlurmProfile {
        SlurmProfile {
            schema: SLURM_PROFILE_SCHEMA.to_string(),
            enabled: true,
            provider: ProviderProfileId::new("slurm-remote-v1")
                .unwrap_or_else(|_| unreachable!()),
            cluster_name: Some("cluster-a".to_string()),
            executables: SlurmExecutablePaths {
                sbatch_path: self.sbatch_path.clone(),
                squeue_path: self.squeue_path.clone(),
                scancel_path: self.scancel_path.clone(),
                sacct_path: None,
            },
            allowed_partitions: BTreeSet::from(["ci-batch".to_string()]),
            allowed_accounts: BTreeSet::from(["kiln-ci".to_string()]),
            allowed_working_dir_prefixes: vec![self.work_dir.clone()],
            log_root: Some(self.log_dir.clone()),
            capabilities: BTreeSet::from([
                RemoteCapability::CasUpload,
                RemoteCapability::Execute,
                RemoteCapability::Poll,
                RemoteCapability::Wait,
                RemoteCapability::Cancel,
                RemoteCapability::LogStream,
            ]),
            bounds: SlurmPolicyBounds {
                command_timeout_ms: 2_000,
                poll_interval_ms: 10,
                teardown_timeout_ms: 500,
                max_wait_polls: 5,
                ..SlurmPolicyBounds::default()
            },
        }
    }

    fn spec(&self) -> SlurmSubmissionSpec {
        SlurmSubmissionSpec {
            program: PathBuf::from("/usr/bin/env"),
            args: vec!["--arg".to_string(), "literal; $PATH".to_string()],
            working_dir: self.work_dir.clone(),
            resources: SlurmResources {
                partition: "ci-batch".to_string(),
                account: "kiln-ci".to_string(),
                cpus_per_task: 2,
                memory_mb: 512,
                time_limit_minutes: 15,
                nodes: 1,
                ntasks: 1,
                gres: None,
                array_spec: None,
            },
            requested_features: BTreeSet::from([
                SlurmRequestedFeature::BatchSubmit,
                SlurmRequestedFeature::JobPoll,
                SlurmRequestedFeature::JobWait,
                SlurmRequestedFeature::JobCancel,
                SlurmRequestedFeature::LogRead,
            ]),
            interactive_pty: false,
            live_attach: false,
            guest_networking: false,
            stdout_path: Some(self.log_dir.join("9001.out")),
            stderr_path: Some(self.log_dir.join("9001.err")),
        }
    }
}

fn content(ch: char) -> ContentId {
    ContentId::from_blake3_hex(ch.to_string().repeat(BLAKE3_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn sample_action(effect: &str, attempt: &str) -> RemoteAction {
    RemoteAction {
        effect_id: EffectId::new(effect).unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new(attempt).unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("rev-1").unwrap_or_else(|_| unreachable!()),
        source_or_candidate: content('a'),
        command: content('b'),
        environment: content('c'),
        platform: content('d'),
        input_root: ExternalRef::new("kiln", "input-root", "root")
            .unwrap_or_else(|_| unreachable!()),
        canonical_input_root: content('e'),
        declared_outputs: Vec::new(),
        timeout_policy: content('f'),
        provider: ProviderProfileId::new("slurm-remote-v1").unwrap_or_else(|_| unreachable!()),
    }
}

fn required_digest() -> ExternalRef {
    ExternalRef::new("kiln", "digest", "blake3").unwrap_or_else(|_| unreachable!())
}

#[test]
fn valid_submit_poll_complete_and_bounded_log_read() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let log_file = cluster.log_dir.join("9001.out");
    fs::write(&log_file, b"hello from slurm node\n").unwrap_or_else(|_| unreachable!());

    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let remote_profile = adapter.remote_profile().unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-happy", "att-1");
    let spec = cluster.spec();
    let correlation = adapter
        .stage_action(&action, spec)
        .unwrap_or_else(|_| unreachable!());

    let obs = dispatch(&remote_profile, &action, &required_digest(), 256, &mut adapter)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(obs.outcome, Outcome::Pending);
    let op = obs.operation.clone().unwrap_or_else(|| unreachable!());
    assert_eq!(op.provider_namespace.as_str(), "slurm");
    assert_eq!(op.native_kind.as_str(), "job");
    assert_eq!(op.native_id.as_str(), "9001");
    assert_eq!(obs.action, correlation.action_ref);

    let recorded_argv =
        fs::read_to_string(&cluster.sbatch_argv_file).unwrap_or_else(|_| unreachable!());
    assert!(recorded_argv.contains("--parsable"));
    assert!(recorded_argv.contains("--export=NONE"));
    assert!(recorded_argv.contains("literal; $PATH"));
    assert!(recorded_argv.contains(&format!("--job-name={}", correlation.job_name)));

    // Poll 1 -> Pending
    let (phase1, poll1) = adapter.poll_phase(&op).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase1, SlurmJobPhase::Pending);
    assert!(matches!(
        poll1,
        RemoteTransportObservation::Observation(ref o) if o.outcome == Outcome::Pending
    ));

    // Poll 2 -> Running
    let (phase2, poll2) = adapter.poll_phase(&op).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase2, SlurmJobPhase::Running);
    assert!(matches!(
        poll2,
        RemoteTransportObservation::Observation(ref o) if o.outcome == Outcome::Pending
    ));

    // Poll 3 via kiln_adapter_remote -> Succeeded
    let poll3 = observe_operation(&remote_profile, &op, ObservationMode::Poll, &mut adapter)
        .unwrap_or_else(|_| unreachable!());
    match poll3 {
        RemoteTransportObservation::Observation(o) => {
            assert_eq!(o.outcome, Outcome::Success);
            assert!(o.required_outputs_present);
            let log_ref = o.logs.first().cloned().unwrap_or_else(|| unreachable!());
            let log_bytes = read_bounded_log(&remote_profile, &log_ref, &mut adapter)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(log_bytes, b"hello from slurm node\n");
        }
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. } => unreachable!(),
    }
}

#[test]
fn wait_polls_until_terminal_success() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let remote_profile = adapter.remote_profile().unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-wait", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    let submitted = dispatch(&remote_profile, &action, &required_digest(), 256, &mut adapter)
        .unwrap_or_else(|_| unreachable!());
    let op = submitted.operation.unwrap_or_else(|| unreachable!());

    let waited = observe_operation(&remote_profile, &op, ObservationMode::Wait, &mut adapter)
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        waited,
        RemoteTransportObservation::Observation(o) if o.outcome == Outcome::Success
    ));
}

#[test]
fn cancel_invokes_scancel_and_returns_cancelled() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let remote_profile = adapter.remote_profile().unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-cancel", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    let submitted = dispatch(&remote_profile, &action, &required_digest(), 256, &mut adapter)
        .unwrap_or_else(|_| unreachable!());
    let op = submitted.operation.unwrap_or_else(|| unreachable!());

    let cancelled =
        cancel_operation(&remote_profile, &op, &mut adapter).unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        cancelled,
        RemoteTransportObservation::Observation(o) if o.outcome == Outcome::Cancelled
    ));
    let recorded =
        fs::read_to_string(&cluster.scancel_invoked_file).unwrap_or_else(|_| unreachable!());
    assert_eq!(recorded.trim(), "9001");
}

#[test]
fn disabled_by_default_and_unsupported_capabilities_refuse_before_external_effect() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();

    // Off-by-default refuses construction
    let mut disabled = cluster.profile();
    disabled.enabled = false;
    assert!(SlurmAdapter::new(disabled).is_err());

    // Unsupported PTY / GPU / Download refuse before invoking any CLI script
    let mut profile = cluster.profile();
    profile.capabilities.remove(&RemoteCapability::Wait);
    let mut adapter = SlurmAdapter::new(profile).unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-cap", "att-1");

    let mut pty_spec = cluster.spec();
    pty_spec.interactive_pty = true;
    assert!(adapter.stage_action(&action, pty_spec).is_err());

    let mut gpu_spec = cluster.spec();
    gpu_spec.resources.gres = Some("gpu:1".to_string());
    assert!(adapter.stage_action(&action, gpu_spec).is_err());

    let op = ExternalRef::new("slurm", "job", "9001").unwrap_or_else(|_| unreachable!());
    assert!(adapter.wait(&op).is_err());
    assert!(adapter.download(&op, 128).is_err());

    assert!(!cluster.sbatch_count_file.exists());
    assert!(!cluster.squeue_invoked_file.exists());
    assert!(!cluster.scancel_invoked_file.exists());
}

#[test]
fn replayed_execute_does_not_duplicate_sbatch_and_conflicting_replay_fails() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-replay", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action).unwrap_or_else(|_| unreachable!());

    let first = adapter.execute(&action).unwrap_or_else(|_| unreachable!());
    let second = adapter.execute(&action).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    let count = fs::read_to_string(&cluster.sbatch_count_file).unwrap_or_else(|_| unreachable!());
    assert_eq!(count.trim(), "1");

    // Replayed trigger with changed attempt_id for same effect_id must fail before sbatch
    let mutated = sample_action("eff-replay", "att-2");
    assert!(adapter.stage_action(&mutated, cluster.spec()).is_err());
    assert!(adapter.execute(&mutated).is_err());
    let count_after =
        fs::read_to_string(&cluster.sbatch_count_file).unwrap_or_else(|_| unreachable!());
    assert_eq!(count_after.trim(), "1");
}

#[test]
fn stale_and_mismatched_job_identities_are_rejected() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());

    let unknown_op = ExternalRef::new("slurm", "job", "99999").unwrap_or_else(|_| unreachable!());
    assert!(adapter.poll(&unknown_op).is_err());
    assert!(adapter.cancel(&unknown_op).is_err());
    assert!(!cluster.squeue_invoked_file.exists());
    assert!(!cluster.scancel_invoked_file.exists());

    // Now register job 9001, but make squeue return job 1234 (mismatch)
    let action = sample_action("eff-mismatch", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action).unwrap_or_else(|_| unreachable!());
    let obs = adapter.execute(&action).unwrap_or_else(|_| unreachable!());
    let op = match obs {
        RemoteTransportObservation::Observation(o) => o.operation.unwrap_or_else(|| unreachable!()),
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. } => unreachable!(),
    };
    cluster.write_script(&cluster.squeue_path, "#!/bin/sh\nprintf '1234|COMPLETED|0:0|node|None\\n'\n");
    assert!(matches!(adapter.poll(&op), Err(PortFailure::Domain(_))));
}

#[test]
fn malformed_sbatch_and_squeue_outputs_fail_closed() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    cluster.write_script(&cluster.sbatch_path, "#!/bin/sh\nprintf 'not-a-job-id\\n'\n");

    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-malformed-sbatch", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action).unwrap_or_else(|_| unreachable!());
    assert!(matches!(adapter.execute(&action), Err(PortFailure::Domain(_))));

    // Valid submit, then malformed squeue output
    cluster.install_happy_scripts();
    let action2 = sample_action("eff-malformed-squeue", "att-1");
    adapter
        .stage_action(&action2, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action2).unwrap_or_else(|_| unreachable!());
    let op = match adapter.execute(&action2).unwrap_or_else(|_| unreachable!()) {
        RemoteTransportObservation::Observation(o) => o.operation.unwrap_or_else(|| unreachable!()),
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. } => unreachable!(),
    };
    cluster.write_script(&cluster.squeue_path, "#!/bin/sh\nprintf 'garbage-without-delimiter\\n'\n");
    assert!(matches!(adapter.poll(&op), Err(PortFailure::Domain(_))));
}

#[test]
fn explicit_failure_classifications_and_unknown_outcomes() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut adapter = SlurmAdapter::new(cluster.profile()).unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-states", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action).unwrap_or_else(|_| unreachable!());
    let op = match adapter.execute(&action).unwrap_or_else(|_| unreachable!()) {
        RemoteTransportObservation::Observation(o) => o.operation.unwrap_or_else(|| unreachable!()),
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. } => unreachable!(),
    };

    for (line, expected_phase, expected_outcome) in [
        (
            "9001|FAILED|1:0|node-01|NonZeroExitCode",
            SlurmJobPhase::Failed {
                exit_code: Some(1),
                signal: None,
            },
            Outcome::DomainFailure,
        ),
        (
            "9001|OUT_OF_MEMORY|0:9|node-01|OutOfMemory",
            SlurmJobPhase::OutOfMemory,
            Outcome::DomainFailure,
        ),
        (
            "9001|TIMEOUT|0:0|node-01|TimeLimit",
            SlurmJobPhase::TimedOut,
            Outcome::TimedOut,
        ),
        (
            "9001|NODE_FAIL|0:0|node-01|NodeFail",
            SlurmJobPhase::NodeFail,
            Outcome::ProviderError,
        ),
        (
            "9001|CANCELLED by 1000|0:0|node-01|None",
            SlurmJobPhase::Cancelled,
            Outcome::Cancelled,
        ),
        (
            "",
            SlurmJobPhase::Unknown { raw_state: None },
            Outcome::Unknown,
        ),
        (
            "9001|UNKNOWN_STATE||node-01|None",
            SlurmJobPhase::Unknown {
                raw_state: Some("UNKNOWN_STATE".to_string()),
            },
            Outcome::Unknown,
        ),
    ] {
        let script = if line.is_empty() {
            "#!/bin/sh\n".to_string()
        } else {
            format!("#!/bin/sh\nprintf '{line}\\n'\n")
        };
        cluster.write_script(&cluster.squeue_path, &script);
        let (phase, obs) = adapter.poll_phase(&op).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, expected_phase);
        match obs {
            RemoteTransportObservation::Observation(o) => assert_eq!(o.outcome, expected_outcome),
            RemoteTransportObservation::DisconnectedAfterAcceptance { .. } => unreachable!(),
        }
    }

    // Output limit exceeded -> ambiguous DisconnectedAfterAcceptance (Unknown)
    let mut tiny_stdout_profile = cluster.profile();
    tiny_stdout_profile.bounds.max_stdout_bytes = 8;
    let mut tiny_adapter = SlurmAdapter::new(tiny_stdout_profile).unwrap_or_else(|_| unreachable!());
    let op2 = ExternalRef::new("slurm", "job", "9001").unwrap_or_else(|_| unreachable!());
    tiny_adapter
        .restore_operation(&action, cluster.spec(), &op2, Outcome::Pending)
        .unwrap_or_else(|_| unreachable!());
    cluster.write_script(
        &cluster.squeue_path,
        "#!/bin/sh\nprintf '9001|RUNNING||node-with-very-long-output|None\\n'\n",
    );
    let ambiguous = tiny_adapter.poll(&op2).unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        ambiguous,
        RemoteTransportObservation::DisconnectedAfterAcceptance { .. }
    ));
}

fn find_sleep_bin() -> PathBuf {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path_var)
        .map(|dir| dir.join("sleep"))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("/bin/sleep"))
}

#[test]
fn timeout_and_provider_failure_preserve_failure_dispositions() {
    let cluster = MockSlurmCluster::new();
    cluster.install_happy_scripts();
    let mut profile = cluster.profile();
    profile.bounds.command_timeout_ms = 50;
    profile.bounds.poll_interval_ms = 10;

    // Timeout on sbatch
    let sleep_bin = find_sleep_bin();
    cluster.write_script(
        &cluster.sbatch_path,
        &format!("#!/bin/sh\n\"{}\" 1\nprintf '9001\\n'\n", sleep_bin.display()),
    );
    let mut adapter = SlurmAdapter::new(profile.clone()).unwrap_or_else(|_| unreachable!());
    let action = sample_action("eff-timeout", "att-1");
    adapter
        .stage_action(&action, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action).unwrap_or_else(|_| unreachable!());
    let err = adapter.execute(&action).err().unwrap_or_else(|| unreachable!());
    assert_eq!(err.disposition(), ProviderFailureDisposition::TimedOut);

    // Non-zero exit provider failure on sbatch and squeue
    cluster.write_script(
        &cluster.sbatch_path,
        "#!/bin/sh\nprintf 'sbatch: slurmctld unreachable\\n' >&2\nexit 1\n",
    );
    let action2 = sample_action("eff-fail", "att-1");
    adapter
        .stage_action(&action2, cluster.spec())
        .unwrap_or_else(|_| unreachable!());
    adapter.upload(&action2).unwrap_or_else(|_| unreachable!());
    let err2 = adapter.execute(&action2).err().unwrap_or_else(|| unreachable!());
    assert_eq!(err2.disposition(), ProviderFailureDisposition::ProviderFailure);

    cluster.write_script(
        &cluster.squeue_path,
        "#!/bin/sh\nprintf 'slurm_load_jobs error\\n' >&2\nexit 1\n",
    );
    let action3 = sample_action("eff-squeue-fail", "att-1");
    let op = ExternalRef::new("slurm", "job", "9001").unwrap_or_else(|_| unreachable!());
    adapter
        .restore_operation(&action3, cluster.spec(), &op, Outcome::Pending)
        .unwrap_or_else(|_| unreachable!());
    let err3 = adapter.poll(&op).err().unwrap_or_else(|| unreachable!());
    assert_eq!(err3.disposition(), ProviderFailureDisposition::ProviderFailure);
}
