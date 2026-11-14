use super::*;
use kiln_core::implementation::CANDIDATE_HANDOFF_SCHEMA;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{Alias, EffectId};
use kiln_core::workflow::{
    WorkflowControlRequest, WorkflowEffectPlan, WorkflowLogChunk, WorkflowLogReadRequest,
};
use kiln_provider_api::{PortResult, WorkflowPort};
use serde_json::json;

struct NoProviderEffects(CapabilityProfile);
impl WorkflowPort for NoProviderEffects {
    fn profile(&self) -> &CapabilityProfile {
        &self.0
    }
    fn dispatch(&mut self, _: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        unreachable!("verification must not dispatch Lattice")
    }
    fn recover_dispatch(&mut self, _: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        unreachable!("verification must not recover through Lattice")
    }
    fn observe(&mut self, _: &EffectId) -> PortResult<WorkflowObservation> {
        unreachable!("verification must not poll Lattice")
    }
    fn control(&mut self, _: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        unreachable!("verification must not control Lattice")
    }
    fn read_log(&mut self, _: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        unreachable!("verification must not read provider logs")
    }
    fn reconcile(&mut self, _: &EffectId) -> PortResult<WorkflowObservation> {
        unreachable!("verification must not reconcile Lattice")
    }
}

fn content(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64))
        .unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn engine() -> (
    tempfile::TempDir,
    Engine,
    kiln_core::source::SourceResolution,
) {
    let root = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error:?}"));
    let path = root.path().join("repository");
    let repository =
        git2::Repository::init(&path).unwrap_or_else(|error| unreachable!("{error:?}"));
    let blob = repository
        .blob(b"real source bytes\n")
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    builder
        .insert("source.txt", blob, 0o100644)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let tree_id = builder
        .write()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let signature = git2::Signature::new("test", "test@example.invalid", &git2::Time::new(0, 0))
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let commit = repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "baseline",
            &tree,
            &[],
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let operation = identity(&(
        "kiln.independent-verification.operation.v1",
        content('a'),
        "job",
    ))
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let profile: crate::profile::ServiceProfile = serde_json::from_value(json!({
        "schema":"kiln.web-service.v1","service_id":"verification-tests","listen_address":"127.0.0.1:9443",
        "origin":"https://localhost:9443","tls":{"certificate_path":root.path().join("cert.pem"),"private_key_path":root.path().join("key.pem"),"min_version":"TLS1.3"},
        "operator_token_file":root.path().join("operator.token"),"worker_token_file":root.path().join("worker.token"),"state_directory":root.path().join("state"),
        "projects":[{"id":"fixture","label":"Fixture","repository_path":path,"reference":"refs/heads/main","retry_failed":2,
            "source_limits":{"max_entries":32,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":16384},
            "workflow":{"socket_path":root.path().join("lattice.sock"),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],
                "transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},
                "adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},"policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64",
                "nodes":[{"id":"job","kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":operation.as_str()}],"edges":[],
                "controls":{"cancel":true,"pause":false,"resume":false},"bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}}],
        "bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,"timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":4,
            "max_artifact_bytes":1024,"max_artifacts":2,"max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,"max_requests":32,"max_events_per_run":8,"max_state_bytes":1048576}
    })).unwrap_or_else(|error| unreachable!("{error:?}"));
    let project = profile.projects.first().unwrap_or_else(|| unreachable!());
    let provider =
        ProviderProfileId::new("test-git").unwrap_or_else(|error| unreachable!("{error:?}"));
    let adapter = LocalGitAdapter::new(
        project.repository_path.clone(),
        commit.to_string(),
        GitSourceProfile::pinned_local(provider.clone(), project.source_limits.clone()),
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let source =
        resolve_and_acquire(&adapter, provider).unwrap_or_else(|error| unreachable!("{error:?}"));
    let capability = project
        .execution_capabilities(profile.bounds.max_runs)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::from([(
        "fixture".into(),
        Box::new(NoProviderEffects(capability)) as Box<dyn WorkflowPort + Send>,
    )]);
    let mut engine =
        Engine::open_with_ports(profile, ports).unwrap_or_else(|error| unreachable!("{error:?}"));
    engine.state.enrolled.insert("fixture".into());
    (root, engine, source)
}

// This fixture exercises canonical journal/projection and effect isolation only;
// actual verifier execution is covered by kiln-implementer's real local fixtures.
fn record(
    engine: &Engine,
    source: kiln_core::source::SourceResolution,
    outcome: Outcome,
) -> RunRecord {
    let project = engine
        .profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.into(),
        work_identity: content('b'),
        work_revision: 1,
        attempt_identity: content('c'),
        fence: 1,
        repository: "enrolled/repository".into(),
        base: Some(source.exact_revision.clone()),
        candidate: source.exact_revision.clone(),
        tree: match &source.native_facts {
            SourceNativeFacts::Git(facts) => facts.native_tree.clone(),
            _ => unreachable!(),
        },
        changed_paths: vec!["source.txt".into()],
        acceptance_digest: content('d'),
        verification_profile: content('a'),
        toolchain: content('e'),
        environment: content('f'),
        evidence: vec![],
    };
    let run_identity = identity(&(
        "kiln.implementation.canonical-verification.v1",
        &engine.profile.service_id,
        &project.id,
        &handoff,
        &handoff.verification_profile,
    ))
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let pipeline = identity(&project.workflow).unwrap_or_else(|error| unreachable!("{error:?}"));
    let mut plan = workflow_plan(project, &run_identity, &pipeline, None)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    plan.provider_profile =
        ProviderProfileId::new(VERIFIER_PROVIDER).unwrap_or_else(|error| unreachable!("{error:?}"));
    let node_state = match outcome {
        Outcome::Success => WorkflowNodeState::Completed,
        Outcome::Unknown => WorkflowNodeState::Unknown,
        _ => WorkflowNodeState::Failed,
    };
    let execution = ExternalRef::new("kiln", "independent-verification", run_identity.as_str())
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let evidence = b"retained canonical observation fixture";
    let artifact_hash = blake3::hash(evidence).to_hex().to_string();
    let receipt = ExternalRef::new("kiln", "check-evidence", &artifact_hash)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let observation = WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: ExternalRef::new(
            "kiln",
            "verification-profile",
            handoff.verification_profile.as_str(),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}")),
        execution: execution.clone(),
        sequence: 1,
        state: match outcome {
            Outcome::Success => WorkflowState::Completed,
            Outcome::Unknown => WorkflowState::Unknown,
            _ => WorkflowState::Failed,
        },
        terminal: outcome != Outcome::Unknown,
        successful: outcome == Outcome::Success,
        outcome,
        nodes: vec![WorkflowNodeObservation {
            node_id: Alias::new("job").unwrap_or_else(|error| unreachable!("{error:?}")),
            state: node_state,
            terminal: node_state.is_terminal(),
            successful: node_state.is_successful(),
            adapter: ExternalRef::new("kiln", "verifier", "fixture")
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            receipt: node_state.is_terminal().then_some(receipt.clone()),
        }],
        logs: vec![],
        receipt: (outcome != Outcome::Unknown).then_some(receipt),
    };
    admit_workflow_observation(&plan, &observation)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let observation_identity =
        identity(&observation).unwrap_or_else(|error| unreachable!("{error:?}"));
    let independent_receipt = (outcome == Outcome::Success).then(|| content('1'));
    let root = fs::canonicalize(&project.repository_path)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let subject = "2".repeat(64);
    let binding = provenance_binding(
        &engine.profile.service_id,
        &handoff.repository,
        &root,
        &handoff,
        &subject,
        &observation_identity,
        independent_receipt.as_ref(),
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let provenance = VerificationProvenance {
        service_id: engine.profile.service_id.clone(),
        repository_alias: handoff.repository.clone(),
        acquisition_root: root,
        handoff,
        subject_hash: subject,
        observation_identity,
        independent_receipt,
        binding,
    };
    let mut record = RunRecord {
        id: format!("web-verification-{}", run_identity.as_str()),
        ordinal: 1,
        project_id: project.id.clone(),
        pipeline_identity: pipeline,
        source,
        requested_reference: None,
        requested_pull_request: None,
        requested_trigger: None,
        plan: plan.clone(),
        events: vec![],
        event_recorded_at_ms: vec![],
        phase: if outcome == Outcome::Unknown {
            "unknown"
        } else {
            "terminal"
        }
        .into(),
        observation: Some(observation),
        error_code: None,
        superseded_by: None,
        superseded_revision: None,
        artifacts: vec![],
        logs: vec![],
        created_at_ms: None,
        started_at_ms: None,
        completed_at_ms: None,
        parent_run_id: None,
        auto_retry: None,
        selected_nodes: vec![],
        node_timing: BTreeMap::new(),
        reports: vec![],
        step_telemetry: vec![],
        dirty_snapshot: None,
        affected: None,
        scope: None,
        untrusted: None,
        implementation_verification: Some(provenance),
        state: State::default(),
    };
    record
        .apply(Command::StartRun {
            run_id: RunId::new(record.id.clone()).unwrap_or_else(|error| unreachable!("{error:?}")),
            attempt_id: plan.attempt_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            effect_id: plan.effect_id.clone(),
        })
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    if outcome == Outcome::Unknown {
        record
            .mark_unknown()
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    } else {
        record
            .complete(outcome, Some(execution.clone()))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    }
    let artifact_id = identity(&("canonical-verification-fixture-artifact", &record.id))
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    super::super::storage::publish_retained(
        &engine.profile.state_directory.join("artifacts"),
        artifact_id.as_str(),
        evidence,
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    let artifact = ArtifactView {
        id: artifact_id.as_str().into(),
        name: "job-verification.json".into(),
        node_id: "job".into(),
        attempt_id: plan.attempt_id.clone(),
        bytes: evidence.len(),
        blake3: artifact_hash,
        download_path: format!("/api/runs/{}/artifacts/{}", record.id, artifact_id.as_str()),
    };
    record.artifacts.push(artifact.clone());
    record.reports.push(ReportRecord {
        id: artifact_id.as_str().into(),
        name: artifact.name.clone(),
        node_id: "job".into(),
        attempt_id: plan.attempt_id.clone(),
        source_revision: record.source.exact_revision.native_id.as_str().into(),
        execution,
        kind: crate::reports::ReportKind::Provenance,
        artifact: Some(artifact),
        report: None,
        unavailable: None,
        recorded_at_ms: 0,
    });
    validate_record_provenance(&record).unwrap_or_else(|error| unreachable!("{error:?}"));
    record
}

#[test]
fn canonical_verification_preserves_native_source_and_maps_only_bound_alias() {
    for outcome in [Outcome::Success, Outcome::DomainFailure, Outcome::Unknown] {
        let (_root, mut engine, source) = engine();
        let native = source.repository.clone();
        let record = record(&engine, source, outcome);
        let mut record: RunRecord = serde_json::from_slice(
            &serde_json::to_vec(&record).unwrap_or_else(|error| unreachable!("{error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        record
            .restore()
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let id = record.id.clone();
        engine.state.runs.insert(id.clone(), record);
        let facts = engine.code_review_ci();
        let fact = facts.first().unwrap_or_else(|| unreachable!());
        let check = fact.checks.first().unwrap_or_else(|| unreachable!());
        assert_eq!(fact.repository, "enrolled/repository");
        assert_eq!(fact.success, outcome == Outcome::Success);
        assert_eq!(
            check.outcome,
            match outcome {
                Outcome::Success => "pass",
                Outcome::Unknown => "unknown",
                _ => "fail",
            }
        );
        assert!(check.artifact_digest.is_some());
        let current = engine.state.runs.get(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(current.source.repository, native);
        let mut forged = current.clone();
        forged
            .implementation_verification
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .repository_alias = "forged/repository".into();
        assert!(validate_record_provenance(&forged).is_err());
        engine.state.runs.insert(id.clone(), forged);
        let facts = engine.code_review_ci();
        let fact = facts.first().unwrap_or_else(|| unreachable!());
        assert!(!fact.success);
        assert_eq!(fact.repository, native.native_id.as_str());
        let run = engine
            .state
            .runs
            .get_mut(&id)
            .unwrap_or_else(|| unreachable!());
        run.implementation_verification = None;
        assert!(validate_record_provenance(run).is_err());
        assert!(
            engine
                .code_review_ci()
                .first()
                .is_some_and(|fact| !fact.success)
        );
    }
}

#[test]
fn verifier_unknown_failed_and_success_records_never_poll_retry_or_control_lattice() {
    for outcome in [Outcome::Unknown, Outcome::DomainFailure, Outcome::Success] {
        let (_root, mut engine, source) = engine();
        let record = record(&engine, source, outcome);
        let id = record.id.clone();
        let attempt = record.plan.attempt_id.clone();
        let revision = record.state.revision;
        let execution = record
            .observation
            .as_ref()
            .unwrap_or_else(|| unreachable!())
            .execution
            .clone();
        engine.state.runs.insert(id.clone(), record);
        assert!(
            engine
                .state
                .runs
                .get(&id)
                .is_some_and(|run| !run.capabilities().cancel)
        );
        engine
            .refresh(&id)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        engine
            .auto_retry_failed(&id)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        engine
            .poll()
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let rerun = engine
            .rerun(
                &id,
                &serde_json::to_vec(&json!({"request_id":"rerun-verifier","mode":"exact"}))
                    .unwrap_or_else(|error| unreachable!("{error:?}")),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(rerun.status, 409);
        let control_body = serde_json::to_vec(&json!({"request_id":"control-verifier","action":"cancel",
            "attempt_id":attempt,"expected_revision":revision,"execution":execution,"expected_sequence":1})).unwrap_or_else(|error| unreachable!("{error:?}"));
        let control = engine
            .control(&id, &control_body)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(control.status, 409);
        assert_eq!(engine.state.runs.len(), 1);
    }
}

#[test]
fn native_profile_validator_requires_every_frozen_case_kind_prerequisite_and_cairn_marker() {
    let (_root, engine, _source) = engine();
    let mut profile: VerificationProfile = serde_json::from_value(json!({
        "schema":"kiln.verification-profile.v1","repository":"enrolled/repository",
        "toolchain":"a".repeat(64),"environment_identity":"b".repeat(64),"environment":{},
        "checks":[
            {"id":"job","kind":"test","program":{"path":"/nix/store/configuration-fixture/bin/check","blake3":"c".repeat(64)},
                "arguments":[],"prerequisites":[],"acceptance":["case"],"artifacts":["result"],"timeout_ms":1000},
            {"id":"dependent","kind":"lint","program":{"path":"/nix/store/configuration-fixture/bin/check","blake3":"c".repeat(64)},
                "arguments":[],"prerequisites":["job"],"acceptance":["case"],"artifacts":["lint-result"],"timeout_ms":1000}
        ],"reproducer":null,"protected_paths":["requirements"],"allow_already_satisfied":false,
        "cairn":{"aliases":[],"contract":{},"registry":{"path":"/nix/store/configuration-fixture/registry.json","blake3":"d".repeat(64)},
            "policy":{"path":"/nix/store/configuration-fixture/policy.json","blake3":"e".repeat(64)}}
    })).unwrap_or_else(|error| unreachable!("{error:?}"));
    // These are configuration fixtures only; no immutable executable/material
    // is read and no successful completion or process execution is claimed.
    profile.toolchain = profile
        .derived_toolchain_identity()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    profile.environment_identity = profile
        .derived_environment_identity()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let mut project = engine
        .profile
        .projects
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_err());
    let mut workflow =
        serde_json::to_value(&project.workflow).unwrap_or_else(|error| unreachable!("{error:?}"));
    let rows = [
        ("job", "test"),
        ("dependent", "lint"),
        (NATIVE_CHECK, "cairn-agent-completion-v2"),
    ];
    *workflow.get_mut("nodes").unwrap_or_else(|| unreachable!()) = json!(rows.into_iter().map(|(id, kind)| json!({
        "id":id,"kind":kind,"effect_class":"test","adapter_kind":"local",
        "operation_identity":Engine::implementation_verification_operation(&profile,id).unwrap_or_else(|error| unreachable!("{error:?}")).as_str()
    })).collect::<Vec<_>>());
    *workflow.get_mut("edges").unwrap_or_else(|| unreachable!()) = json!([
        {"source":"job","target":"dependent"},
        {"source":"job","target":NATIVE_CHECK}, {"source":"dependent","target":NATIVE_CHECK}
    ]);
    project.workflow =
        serde_json::from_value(workflow.clone()).unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_err());
    project
        .workflow
        .nodes
        .sort_by(|left, right| left.id.cmp(&right.id));
    project.workflow.edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
    });
    workflow =
        serde_json::to_value(&project.workflow).unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_ok());
    let mut missing = workflow.clone();
    missing
        .get_mut("nodes")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!())
        .pop();
    project.workflow =
        serde_json::from_value(missing).unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_err());
    let mut changed_kind = workflow.clone();
    *changed_kind
        .get_mut("nodes")
        .and_then(|value| value.as_array_mut())
        .and_then(|nodes| nodes.get_mut(1))
        .and_then(|node| node.get_mut("kind"))
        .unwrap_or_else(|| unreachable!()) = json!("unrelated");
    project.workflow =
        serde_json::from_value(changed_kind).unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_err());
    workflow
        .get_mut("edges")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!())
        .pop();
    project.workflow =
        serde_json::from_value(workflow).unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(Engine::validate_implementation_verification_project(&profile, &project).is_err());
}
