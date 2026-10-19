mod support;
use support::*;

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path};

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use kiln_adapter_pijul::{LocalPijulAdapter, PijulSourceProfile};
use kiln_core::domain::{Command, CommandEnvelope, Outcome, State, decide, evolve};
use kiln_core::source::{PijulEntryKind, PijulSnapshotEntry};
use kiln_core::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CandidateId, CommandId, ContentId, Diagnostic, EffectId,
    ExternalRef, Limits, PipelineRevisionId, PolicyId, ProviderProfileId, RunId,
};
use kiln_core::workflow::{
    WorkflowBounds, WorkflowControls, WorkflowEffectClass, WorkflowEffectPlan, WorkflowNode,
    WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation, WorkflowSourceContext,
    WorkflowState, admit_workflow_observation,
};
use kiln_provider_api::{PijulReadObservation, PijulReadRequest, PijulRepositoryPort, PortFailure};

fn core<T>(result: Result<T, Diagnostic>) -> anyhow::Result<T> {
    result.map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))
}

fn port<T>(result: Result<T, PortFailure>) -> anyhow::Result<T> {
    result.map_err(|error| anyhow::anyhow!("{error:?}"))
}

fn materialize(checkout: &Path, entries: &[PijulSnapshotEntry]) -> io::Result<()> {
    for entry in entries {
        let path = Path::new(&entry.path);
        for component in path.components() {
            match component {
                Component::Normal(part) if part != OsStr::new(".pijul") => {}
                _ => return Err(io::Error::other("unsafe Pijul snapshot entry path")),
            }
        }
        let target = checkout.join(path);
        match entry.kind {
            PijulEntryKind::Directory => fs::create_dir_all(&target)?,
            PijulEntryKind::Regular | PijulEntryKind::Executable => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target, &entry.bytes)?;
                let mode = if entry.kind == PijulEntryKind::Executable {
                    0o755
                } else {
                    0o644
                };
                fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
            }
            PijulEntryKind::Unsupported => {
                return Err(io::Error::other("unsupported Pijul snapshot entry"));
            }
        }
    }
    Ok(())
}

fn run_command(checkout: &Path, script: &str) -> io::Result<bool> {
    let policy = OutcomePolicy::new(vec![0], true, true)
        .map_err(|_| io::Error::other("fixture outcome policy is invalid"))?;
    let output = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program: Path::new("/bin/sh").to_path_buf(),
            args: vec!["-c".into(), script.into()],
            current_dir: checkout.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: 5_000,
            stdin_max_bytes: 1,
            stdout_max_bytes: 4_096,
            stderr_max_bytes: 4_096,
            poll_interval_ms: 10,
            teardown_timeout_ms: 2_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: policy,
    })
    .map_err(|_| io::Error::other("bounded Pijul fixture command failed to run"))?;
    match output.disposition {
        Disposition::Succeeded => Ok(true),
        Disposition::ExitFailed => Ok(false),
        _ => Err(io::Error::other(
            "bounded Pijul fixture command did not complete normally",
        )),
    }
}

fn plan(observation: &PijulReadObservation, suffix: &str) -> anyhow::Result<WorkflowEffectPlan> {
    Ok(WorkflowEffectPlan {
        effect_id: core(EffectId::new(&format!("pijul-e2e-effect-{suffix}")))?,
        attempt_id: core(AttemptId::new(&format!("pijul-e2e-attempt-{suffix}")))?,
        candidate_id: core(CandidateId::new(&format!("pijul-e2e-candidate-{suffix}")))?,
        pipeline_revision_id: core(PipelineRevisionId::new(&format!(
            "pijul-e2e-pipeline-{suffix}"
        )))?,
        workflow_policy_id: core(PolicyId::new(&format!("pijul-e2e-policy-{suffix}")))?,
        provider_profile: core(ProviderProfileId::new("pijul-source-v1"))?,
        idempotency_key: core(ContentId::from_blake3_hex("c".repeat(64)))?,
        platform: core(Alias::new("linux-x86_64"))?,
        source: Some(WorkflowSourceContext {
            provider: core(Alias::new("pijul"))?,
            repository: observation.repository.clone(),
            revision: observation.observed_state_before.clone(),
        }),
        nodes: vec![WorkflowNode {
            id: core(Alias::new("test"))?,
            kind: core(Alias::new("test"))?,
            effect_class: WorkflowEffectClass::Test,
            adapter_kind: core(Alias::new("local"))?,
            operation_identity: core(ContentId::from_blake3_hex("d".repeat(64)))?,
        }],
        edges: Vec::new(),
        controls: WorkflowControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        bounds: WorkflowBounds {
            max_nodes: 4,
            max_edges: 4,
            max_log_references: 4,
            max_log_read_bytes: 1_024,
            max_total_log_bytes: 4_096,
        },
    })
}

fn observation(
    plan: &WorkflowEffectPlan,
    successful: bool,
    suffix: &str,
) -> anyhow::Result<WorkflowObservation> {
    let node = plan
        .nodes
        .first()
        .ok_or_else(|| anyhow::anyhow!("Pijul E2E plan has no node"))?;
    let (state, node_state, outcome) = if successful {
        (
            WorkflowState::Completed,
            WorkflowNodeState::Completed,
            Outcome::Success,
        )
    } else {
        (
            WorkflowState::Failed,
            WorkflowNodeState::Failed,
            Outcome::DomainFailure,
        )
    };
    let receipt = Some(core(ExternalRef::new(
        "pijul",
        "receipt",
        &format!("pijul-e2e-receipt-{suffix}"),
    ))?);
    Ok(WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: core(ExternalRef::new(
            "pijul",
            "workflow",
            &format!("pijul-e2e-workflow-{suffix}"),
        ))?,
        execution: core(ExternalRef::new(
            "pijul",
            "execution",
            &format!("pijul-e2e-execution-{suffix}"),
        ))?,
        sequence: 1,
        state,
        terminal: true,
        successful,
        outcome,
        nodes: vec![WorkflowNodeObservation {
            node_id: node.id.clone(),
            state: node_state,
            terminal: true,
            successful,
            adapter: core(ExternalRef::new("pijul", "adapter", "local"))?,
            receipt: receipt.clone(),
        }],
        logs: Vec::new(),
        receipt,
    })
}

fn apply(state: &State, command: &CommandEnvelope) -> anyhow::Result<State> {
    let decision = core(decide(state, command, Limits::default()))?;
    let mut next = state.clone();
    for event in &decision.events {
        next = core(evolve(&next, event))?;
    }
    Ok(next)
}

fn run_case(
    entries: &[PijulSnapshotEntry],
    source: &PijulReadObservation,
    suffix: &str,
    script: &str,
    expected: Outcome,
) -> anyhow::Result<()> {
    let checkout = tempfile::tempdir()?;
    materialize(checkout.path(), entries)?;
    let successful = run_command(checkout.path(), script)?;
    let plan = plan(source, suffix)?;
    let observation = observation(&plan, successful, suffix)?;
    let admitted = core(admit_workflow_observation(&plan, &observation))?;
    assert_eq!(admitted, expected);

    let run_id = core(RunId::new(&format!("pijul-e2e-run-{suffix}")))?;
    let start = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: core(CommandId::new(&format!("pijul-e2e-start-{suffix}")))?,
        expected_revision: 0,
        command: Command::StartRun {
            run_id: run_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            effect_id: plan.effect_id.clone(),
        },
    };
    let state = apply(&State::default(), &start)?;
    let complete = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: core(CommandId::new(&format!("pijul-e2e-complete-{suffix}")))?,
        expected_revision: state.revision,
        command: Command::CompleteAttempt {
            attempt_id: plan.attempt_id.clone(),
            outcome: admitted,
            external_reference: Some(observation.execution.clone()),
            products: Vec::new(),
            metrics: Vec::new(),
        },
    };
    let state = apply(&state, &complete)?;
    let run = state
        .runs
        .get(&run_id)
        .ok_or_else(|| anyhow::anyhow!("Pijul E2E run is missing from state"))?;
    assert_eq!(run.outcome, expected);
    Ok(())
}

// r[verify kiln.source.pijul_e2e]
// r[verify kiln.source.pijul_e2e.scenario.pass]
// r[verify kiln.source.pijul_e2e.scenario.fail]
// r[verify kiln.source.pijul_e2e.scenario.mapping]
#[test]
fn pijul_snapshot_flows_through_reviewed_command_and_ci_transition() -> anyhow::Result<()> {
    let executable = executable()?;
    let (_temporary, repository) = create_repository(&executable, "full-e2e", "full e2e history")?;
    let profile = PijulSourceProfile::pinned_local();
    let adapter = port(LocalPijulAdapter::new(
        repository,
        FIXTURE_CHANNEL.to_string(),
        profile.clone(),
    ))?;
    let channel = core(ExternalRef::new("pijul", "channel", FIXTURE_CHANNEL))?;
    let request = PijulReadRequest {
        channel: channel.clone(),
        limits: profile.limits.clone(),
    };
    let observation = port(adapter.resolve_and_acquire(&request))?;
    assert_eq!(
        observation.observed_state_before,
        observation.observed_state_after
    );
    let file = observation
        .entries
        .iter()
        .find(|entry| entry.path == FIXTURE_FILE)
        .ok_or_else(|| anyhow::anyhow!("Pijul fixture file is missing from the snapshot"))?;
    assert_eq!(file.kind, PijulEntryKind::Regular);
    assert_eq!(file.bytes.as_slice(), b"fixture content\n");
    assert!(
        observation
            .entries
            .iter()
            .all(|entry| !entry.path.starts_with(".pijul"))
    );

    let resolution = port(kiln_adapter_pijul::resolve_and_acquire(
        &adapter,
        channel,
        profile.id.clone(),
        &profile,
    ))?;
    assert_eq!(resolution.exact_revision, observation.observed_state_before);
    assert_eq!(resolution.repository, observation.repository);
    assert_eq!(resolution.source_provider, profile.id);

    run_case(
        &observation.entries,
        &observation,
        "pass",
        "IFS= read -r value < file.txt; test \"$value\" = 'fixture content' && test ! -e .pijul",
        Outcome::Success,
    )?;
    run_case(
        &observation.entries,
        &observation,
        "fail",
        "IFS= read -r value < file.txt; test \"$value\" = 'fixture content' && exit 23",
        Outcome::DomainFailure,
    )?;
    Ok(())
}

// r[verify kiln.source.pijul_e2e.scenario.unsafe]
#[test]
fn materialization_rejects_unsafe_snapshot_entries() -> anyhow::Result<()> {
    let checkout = tempfile::tempdir()?;
    for path in ["../escape", "/absolute", ".pijul/db", "./file"] {
        let entry = PijulSnapshotEntry {
            path: path.to_string(),
            kind: PijulEntryKind::Regular,
            bytes: b"unsafe".to_vec(),
        };
        assert!(
            materialize(checkout.path(), &[entry]).is_err(),
            "unsafe path was materialized: {path}"
        );
    }
    let unsupported = PijulSnapshotEntry {
        path: "file".to_string(),
        kind: PijulEntryKind::Unsupported,
        bytes: Vec::new(),
    };
    assert!(materialize(checkout.path(), &[unsupported]).is_err());
    Ok(())
}
