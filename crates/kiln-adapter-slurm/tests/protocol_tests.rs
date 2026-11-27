use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_adapter_slurm::{
    SLURM_PROFILE_SCHEMA, SlurmExecutablePaths, SlurmJobPhase, SlurmPolicyBounds, SlurmProfile,
    SlurmRequestedFeature, SlurmResources, SlurmSubmissionSpec, admit_profile, admit_submission,
    build_sbatch_args, classify_slurm_state, parse_sacct_output, parse_sbatch_output,
    parse_squeue_output, validate_slurm_operation_ref,
};
use kiln_core::domain::Outcome;
use kiln_core::remote::{RemoteAction, RemoteCapability};
use kiln_core::types::{
    AttemptId, BLAKE3_HEX_BYTES, ContentId, EffectId, ExternalRef, PipelineRevisionId,
    ProviderProfileId,
};

fn content(ch: char) -> ContentId {
    ContentId::from_blake3_hex(ch.to_string().repeat(BLAKE3_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn enabled_profile() -> SlurmProfile {
    SlurmProfile {
        schema: SLURM_PROFILE_SCHEMA.to_string(),
        enabled: true,
        provider: ProviderProfileId::new("slurm-remote-v1").unwrap_or_else(|_| unreachable!()),
        cluster_name: Some("cluster-a".to_string()),
        executables: SlurmExecutablePaths {
            sbatch_path: PathBuf::from("/usr/bin/sbatch"),
            squeue_path: PathBuf::from("/usr/bin/squeue"),
            scancel_path: PathBuf::from("/usr/bin/scancel"),
            sacct_path: Some(PathBuf::from("/usr/bin/sacct")),
        },
        allowed_partitions: BTreeSet::from(["ci-batch".to_string()]),
        allowed_accounts: BTreeSet::from(["kiln-ci".to_string()]),
        allowed_working_dir_prefixes: vec![PathBuf::from("/var/lib/kiln/slurm-work")],
        log_root: Some(PathBuf::from("/var/lib/kiln/slurm-logs")),
        capabilities: BTreeSet::from([
            RemoteCapability::CasUpload,
            RemoteCapability::Execute,
            RemoteCapability::Poll,
            RemoteCapability::Wait,
            RemoteCapability::Cancel,
            RemoteCapability::LogStream,
        ]),
        bounds: SlurmPolicyBounds::default(),
    }
}

fn sample_action() -> RemoteAction {
    RemoteAction {
        effect_id: EffectId::new("effect-1").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt-1").unwrap_or_else(|_| unreachable!()),
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

fn sample_spec() -> SlurmSubmissionSpec {
    SlurmSubmissionSpec {
        program: PathBuf::from("/nix/store/aaaa-runner/bin/kiln-runner"),
        args: vec![
            "--flag".to_string(),
            "value with spaces; $(echo nope)".to_string(),
        ],
        working_dir: PathBuf::from("/var/lib/kiln/slurm-work/job-1"),
        resources: SlurmResources {
            partition: "ci-batch".to_string(),
            account: "kiln-ci".to_string(),
            cpus_per_task: 2,
            memory_mb: 1024,
            time_limit_minutes: 30,
            nodes: 1,
            ntasks: 1,
            gres: None,
            array_spec: None,
        },
        requested_features: BTreeSet::from([
            SlurmRequestedFeature::BatchSubmit,
            SlurmRequestedFeature::JobPoll,
        ]),
        interactive_pty: false,
        live_attach: false,
        guest_networking: false,
        stdout_path: Some(PathBuf::from("/var/lib/kiln/slurm-logs/job-1.out")),
        stderr_path: Some(PathBuf::from("/var/lib/kiln/slurm-logs/job-1.err")),
    }
}

#[test]
fn disabled_profile_fails_closed() {
    let profile = SlurmProfile::disabled_default().unwrap_or_else(|_| unreachable!());
    assert!(!profile.enabled);
    let err = admit_profile(&profile).err();
    assert_eq!(
        err.map(|d| d.code),
        Some("slurm_adapter_disabled".to_string())
    );
}

#[test]
fn admission_enforces_allowlists_and_bounds() {
    let profile = enabled_profile();
    let action = sample_action();

    let mut bad_partition = sample_spec();
    bad_partition.resources.partition = "gpu-unlisted".to_string();
    assert_eq!(
        admit_submission(&profile, &action, &bad_partition)
            .err()
            .map(|d| d.code),
        Some("slurm_partition_denied".to_string())
    );

    let mut bad_account = sample_spec();
    bad_account.resources.account = "root".to_string();
    assert_eq!(
        admit_submission(&profile, &action, &bad_account)
            .err()
            .map(|d| d.code),
        Some("slurm_account_denied".to_string())
    );

    let mut too_many_cpus = sample_spec();
    too_many_cpus.resources.cpus_per_task = 999;
    assert_eq!(
        admit_submission(&profile, &action, &too_many_cpus)
            .err()
            .map(|d| d.code),
        Some("slurm_cpu_bound".to_string())
    );

    let mut too_much_mem = sample_spec();
    too_much_mem.resources.memory_mb = 9_999_999;
    assert_eq!(
        admit_submission(&profile, &action, &too_much_mem)
            .err()
            .map(|d| d.code),
        Some("slurm_memory_bound".to_string())
    );

    let mut too_long = sample_spec();
    too_long.resources.time_limit_minutes = 99_999;
    assert_eq!(
        admit_submission(&profile, &action, &too_long)
            .err()
            .map(|d| d.code),
        Some("slurm_time_limit_bound".to_string())
    );
}

#[test]
fn admission_refuses_unsupported_capabilities_and_unsafe_paths() {
    let profile = enabled_profile();
    let action = sample_action();

    let mut pty = sample_spec();
    pty.interactive_pty = true;
    assert!(admit_submission(&profile, &action, &pty).is_err());

    let mut attach = sample_spec();
    attach.live_attach = true;
    assert!(admit_submission(&profile, &action, &attach).is_err());

    let mut net = sample_spec();
    net.guest_networking = true;
    assert!(admit_submission(&profile, &action, &net).is_err());

    let mut feature_pty = sample_spec();
    feature_pty
        .requested_features
        .insert(SlurmRequestedFeature::InteractivePty);
    assert!(admit_submission(&profile, &action, &feature_pty).is_err());

    let mut gres = sample_spec();
    gres.resources.gres = Some("gpu:1".to_string());
    assert!(admit_submission(&profile, &action, &gres).is_err());

    let mut array = sample_spec();
    array.resources.array_spec = Some("1-4".to_string());
    assert!(admit_submission(&profile, &action, &array).is_err());

    let mut multi_node = sample_spec();
    multi_node.resources.nodes = 2;
    assert!(admit_submission(&profile, &action, &multi_node).is_err());

    let mut rel_prog = sample_spec();
    rel_prog.program = PathBuf::from("bin/runner");
    assert!(admit_submission(&profile, &action, &rel_prog).is_err());

    let mut traversal_workdir = sample_spec();
    traversal_workdir.working_dir = PathBuf::from("/var/lib/kiln/slurm-work/../escape");
    assert!(admit_submission(&profile, &action, &traversal_workdir).is_err());

    let mut outside_workdir = sample_spec();
    outside_workdir.working_dir = PathBuf::from("/tmp/outside");
    assert!(admit_submission(&profile, &action, &outside_workdir).is_err());

    let mut ctrl_arg = sample_spec();
    ctrl_arg.args = vec!["bad\narg".to_string()];
    assert!(admit_submission(&profile, &action, &ctrl_arg).is_err());
}

#[test]
fn correlation_is_deterministic_and_sbatch_argv_has_no_shell_interpolation() {
    let profile = enabled_profile();
    let action = sample_action();
    let spec = sample_spec();

    let first = admit_submission(&profile, &action, &spec).unwrap_or_else(|_| unreachable!());
    let second = admit_submission(&profile, &action, &spec).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    assert!(first.job_name.starts_with("kiln-"));
    assert_eq!(first.effect_id, action.effect_id);
    assert_eq!(first.attempt_id, action.attempt_id);

    let mut other_action = sample_action();
    other_action.attempt_id = AttemptId::new("attempt-2").unwrap_or_else(|_| unreachable!());
    let third =
        admit_submission(&profile, &other_action, &spec).unwrap_or_else(|_| unreachable!());
    assert_ne!(first.correlation_id, third.correlation_id);

    let argv = build_sbatch_args(&spec, &first).unwrap_or_else(|_| unreachable!());
    assert!(!argv.iter().any(|arg| arg == "--wrap" || arg == "sh" || arg == "/bin/sh"));
    assert!(argv.contains(&"--parsable".to_string()));
    assert!(argv.contains(&"--export=NONE".to_string()));
    assert!(argv.contains(&"value with spaces; $(echo nope)".to_string()));
    assert!(argv.contains(&format!("--comment={}", first.correlation_id.as_str())));
}

#[test]
fn sbatch_parser_accepts_valid_and_rejects_malformed() {
    let parsed = parse_sbatch_output("42190\n", Some("cluster-a")).unwrap_or_else(|_| unreachable!());
    assert_eq!(parsed.job_id, "42190");
    assert_eq!(parsed.cluster, None);

    let with_cluster = parse_sbatch_output("42190;cluster-a\r\n", Some("cluster-a"))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(with_cluster.job_id, "42190");
    assert_eq!(with_cluster.cluster.as_deref(), Some("cluster-a"));

    assert!(parse_sbatch_output("", None).is_err());
    assert!(parse_sbatch_output("0", None).is_err());
    assert!(parse_sbatch_output("not-a-job", None).is_err());
    assert!(parse_sbatch_output("123\n456", None).is_err());
    assert!(parse_sbatch_output("123;cluster-a;extra", None).is_err());
    assert!(parse_sbatch_output("123;cluster-b", Some("cluster-a")).is_err());
}

#[test]
fn squeue_and_sacct_state_mapping_covers_all_required_states() {
    assert_eq!(classify_slurm_state("PENDING", None), SlurmJobPhase::Pending);
    assert_eq!(classify_slurm_state("CONFIGURING", None), SlurmJobPhase::Pending);
    assert_eq!(SlurmJobPhase::Pending.to_kiln_outcome(), Outcome::Pending);

    assert_eq!(classify_slurm_state("RUNNING", None), SlurmJobPhase::Running);
    assert_eq!(classify_slurm_state("COMPLETING", None), SlurmJobPhase::Running);
    assert_eq!(SlurmJobPhase::Running.to_kiln_outcome(), Outcome::Pending);

    assert_eq!(
        classify_slurm_state("COMPLETED", Some("0:0")),
        SlurmJobPhase::Succeeded
    );
    assert_eq!(
        classify_slurm_state("COMPLETED", Some("0")),
        SlurmJobPhase::Succeeded
    );
    assert_eq!(SlurmJobPhase::Succeeded.to_kiln_outcome(), Outcome::Success);

    assert_eq!(
        classify_slurm_state("COMPLETED", Some("2:0")).to_kiln_outcome(),
        Outcome::DomainFailure
    );
    assert_eq!(
        classify_slurm_state("COMPLETED", None).to_kiln_outcome(),
        Outcome::Unknown
    );
    assert_eq!(
        classify_slurm_state("FAILED", Some("1:0")).to_kiln_outcome(),
        Outcome::DomainFailure
    );
    assert_eq!(
        classify_slurm_state("OUT_OF_MEMORY", Some("0:9")),
        SlurmJobPhase::OutOfMemory
    );
    assert_eq!(
        SlurmJobPhase::OutOfMemory.to_kiln_outcome(),
        Outcome::DomainFailure
    );
    assert_eq!(classify_slurm_state("TIMEOUT", None), SlurmJobPhase::TimedOut);
    assert_eq!(SlurmJobPhase::TimedOut.to_kiln_outcome(), Outcome::TimedOut);
    assert_eq!(classify_slurm_state("NODE_FAIL", None), SlurmJobPhase::NodeFail);
    assert_eq!(
        SlurmJobPhase::NodeFail.to_kiln_outcome(),
        Outcome::ProviderError
    );
    assert_eq!(
        classify_slurm_state("CANCELLED by 1000", None),
        SlurmJobPhase::Cancelled
    );
    assert_eq!(
        SlurmJobPhase::Cancelled.to_kiln_outcome(),
        Outcome::Cancelled
    );
    assert_eq!(
        classify_slurm_state("BIZARRE_UNRECOGNIZED", None).to_kiln_outcome(),
        Outcome::Unknown
    );

    let missing = parse_squeue_output("101", "").unwrap_or_else(|_| unreachable!());
    assert_eq!(missing.phase.to_kiln_outcome(), Outcome::Unknown);

    let valid = parse_squeue_output("101", "101|COMPLETED|0:0|node-01|None\n")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(valid.phase, SlurmJobPhase::Succeeded);
    assert_eq!(valid.node_list.as_deref(), Some("node-01"));

    assert!(parse_squeue_output("101", "202|RUNNING|0:0|node-01|None\n").is_err());
    assert!(parse_squeue_output("101", "101\n").is_err());
    assert!(parse_squeue_output("101", "101|RUNNING\n101|COMPLETED|0:0\n").is_err());

    let sacct = parse_sacct_output(
        "101",
        "101|COMPLETED|0:0|node-01|None\n101.batch|COMPLETED|0:0|node-01|None\n",
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(sacct.phase, SlurmJobPhase::Succeeded);
    assert!(parse_sacct_output("101", "999|COMPLETED|0:0|node-01|None\n").is_err());
}

#[test]
fn operation_ref_validation_rejects_foreign_or_non_numeric_refs() {
    let valid = ExternalRef::new("slurm", "job", "777").unwrap_or_else(|_| unreachable!());
    assert_eq!(validate_slurm_operation_ref(&valid), Ok("777"));

    let wrong_ns = ExternalRef::new("reapi", "job", "777").unwrap_or_else(|_| unreachable!());
    assert!(validate_slurm_operation_ref(&wrong_ns).is_err());

    let wrong_kind = ExternalRef::new("slurm", "node", "777").unwrap_or_else(|_| unreachable!());
    assert!(validate_slurm_operation_ref(&wrong_kind).is_err());

    let non_numeric = ExternalRef::new("slurm", "job", "abc").unwrap_or_else(|_| unreachable!());
    assert!(validate_slurm_operation_ref(&non_numeric).is_err());
}
