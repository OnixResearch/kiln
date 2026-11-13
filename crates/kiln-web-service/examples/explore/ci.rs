//! Native CI state driven by an explicitly non-executing, in-process simulator.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use kiln_core::domain::Outcome;
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{EffectId, ExternalRef};
use kiln_core::workflow::*;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult, WorkflowPort};
use kiln_web_service::engine::{Engine, Role};
use kiln_web_service::profile::ServiceProfile;
use serde_json::{Value, json};

use super::{Example, Result, accepted};
const NODE_IDS: [&str; 3] = ["build", "package", "test"];
const PROJECTS: [(&str, &str); 5] = [
    ("checks", "Core checks · simulated"),
    ("release", "Release evidence · simulated"),
    ("docs", "Documentation · simulated"),
    ("labs", "Unenrolled sandbox · simulated"),
    ("regression", "Failing integration checks · simulated"),
];
const INTERACTIVE_POLLS: u8 = 8;

pub struct Seed {
    pub engine: Engine,
    pub runs: BTreeMap<String, String>,
    pub examples: Vec<Example>,
    heartbeats: super::workers::Heartbeats,
}
impl Seed {
    pub fn poll(&mut self) -> Result {
        self.heartbeats.refresh(&mut self.engine)?;
        // Reservations remain visible and intentionally undelivered. No notification transport is called.
        let _withheld_notifications = self.engine.poll()?;
        Ok(())
    }
}
struct Execution {
    plan: WorkflowEffectPlan,
    state: WorkflowState,
    sequence: u64,
    remaining: Option<u8>,
}
#[derive(Default)]
struct Simulator {
    executions: BTreeMap<String, Execution>,
    interactive: bool,
}
struct Port {
    profile: CapabilityProfile,
    simulator: Arc<Mutex<Simulator>>,
}
fn unavailable(message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "example_simulator".into(),
        message: message.into(),
    })
}
fn reference(kind: &str, name: &str) -> ExternalRef {
    // These constant provider components and bounded Kiln IDs satisfy ExternalRef admission.
    ExternalRef::new("example-simulator", kind, name)
        .unwrap_or_else(|error| unreachable!("Example reference rejected: {error:?}"))
}
fn observation(execution: &Execution) -> WorkflowObservation {
    let plan = &execution.plan;
    let terminal = execution.state.is_terminal();
    let success = execution.state == WorkflowState::Completed;
    let nodes = plan
        .nodes
        .iter()
        .map(|node| {
            let state = match execution.state {
                WorkflowState::Completed => WorkflowNodeState::Completed,
                WorkflowState::Failed if node.id.as_str() == "test" => WorkflowNodeState::Failed,
                WorkflowState::Failed if node.id.as_str() == "package" => {
                    WorkflowNodeState::Skipped
                }
                WorkflowState::Failed => WorkflowNodeState::Completed,
                WorkflowState::Cancelled => WorkflowNodeState::Cancelled,
                WorkflowState::Paused => WorkflowNodeState::Paused,
                WorkflowState::Unknown => WorkflowNodeState::Unknown,
                _ => WorkflowNodeState::Running,
            };
            WorkflowNodeObservation {
                node_id: node.id.clone(),
                state,
                terminal: state.is_terminal(),
                successful: state.is_successful(),
                adapter: reference("adapter", "non-executing"),
                receipt: matches!(
                    state,
                    WorkflowNodeState::Completed
                        | WorkflowNodeState::Failed
                        | WorkflowNodeState::Cancelled
                )
                .then(|| {
                    reference(
                        "receipt",
                        &format!("{}-{}", plan.effect_id.as_str(), node.id.as_str()),
                    )
                }),
            }
        })
        .collect();
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: reference("workflow", "synthetic-checks"),
        execution: reference("execution", plan.effect_id.as_str()),
        sequence: execution.sequence,
        state: execution.state,
        terminal,
        successful: success,
        outcome: match execution.state {
            WorkflowState::Completed => Outcome::Success,
            WorkflowState::Failed => Outcome::DomainFailure,
            WorkflowState::Cancelled => Outcome::Cancelled,
            WorkflowState::Unknown => Outcome::Unknown,
            _ => Outcome::Pending,
        },
        nodes,
        logs: Vec::new(),
        receipt: terminal.then(|| reference("receipt", plan.effect_id.as_str())),
    }
}
impl WorkflowPort for Port {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut simulator = self
            .simulator
            .lock()
            .map_err(|_| unavailable("Simulator lock unavailable"))?;
        let remaining = simulator.interactive.then_some(INTERACTIVE_POLLS);
        let execution = simulator
            .executions
            .entry(plan.effect_id.as_str().into())
            .or_insert_with(|| Execution {
                plan: plan.clone(),
                state: WorkflowState::Running,
                sequence: 1,
                remaining,
            });
        if execution.plan != *plan {
            return Err(unavailable("Effect identity conflict"));
        }
        Ok(observation(execution))
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        self.dispatch(plan)
    }
    fn observe(&mut self, effect: &EffectId) -> PortResult<WorkflowObservation> {
        let mut simulator = self
            .simulator
            .lock()
            .map_err(|_| unavailable("Simulator lock unavailable"))?;
        let execution = simulator
            .executions
            .get_mut(effect.as_str())
            .ok_or_else(|| unavailable("Effect not retained"))?;
        if execution.state == WorkflowState::Running {
            if let Some(remaining) = &mut execution.remaining {
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    execution.state = WorkflowState::Completed;
                    execution.sequence += 1;
                }
            }
        }
        Ok(observation(execution))
    }
    fn reconcile(&mut self, effect: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(effect)
    }
    fn control(&mut self, request: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        let mut simulator = self
            .simulator
            .lock()
            .map_err(|_| unavailable("Simulator lock unavailable"))?;
        let execution = simulator
            .executions
            .get_mut(request.effect_id.as_str())
            .ok_or_else(|| unavailable("Effect not retained"))?;
        if request.expected_sequence != execution.sequence
            || request.execution != reference("execution", request.effect_id.as_str())
            || execution.state.is_terminal()
        {
            return Err(unavailable("Stale simulator control identity"));
        }
        execution.state = match request.action {
            WorkflowControlAction::Cancel => WorkflowState::Cancelled,
            WorkflowControlAction::Pause => WorkflowState::Paused,
            WorkflowControlAction::Resume => WorkflowState::Running,
        };
        execution.sequence += 1;
        Ok(observation(execution))
    }
    fn read_log(&mut self, request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        let simulator = self
            .simulator
            .lock()
            .map_err(|_| unavailable("Simulator lock unavailable"))?;
        let execution = simulator
            .executions
            .get(request.effect_id.as_str())
            .ok_or_else(|| unavailable("Effect not retained"))?;
        if request.execution != reference("execution", request.effect_id.as_str())
            || !execution
                .plan
                .nodes
                .iter()
                .any(|node| node.id == request.node_id)
        {
            return Err(unavailable("Log identity not retained"));
        }
        let content = b"[synthetic example] No source command was executed.\nInspect a seeded completed run for captured stdout, stderr and reports.\n";
        let cursor = usize::try_from(request.cursor)
            .unwrap_or(usize::MAX)
            .min(content.len());
        let end = cursor.saturating_add(request.max_bytes).min(content.len());
        let bytes = content
            .get(cursor..end)
            .ok_or_else(|| unavailable("Log cursor outside retained output"))?
            .to_vec();
        Ok(WorkflowLogChunk {
            reference: WorkflowLogReference {
                node_id: request.node_id.clone(),
                stream: request.stream,
                reference: reference("log", request.effect_id.as_str()),
                cursor: cursor as u64,
                bytes: bytes.len(),
                truncated: false,
                gap_before: false,
                retained: true,
            },
            bytes,
        })
    }
}
fn call(engine: &mut Engine, path: &str, body: Value, role: Role) -> Result<Value> {
    accepted(engine.api("POST", path, &serde_json::to_vec(&body)?, role)?)
        .map_err(|error| format!("CI seed {path}: {error}").into())
}
fn exact<'a>(run: &'a Value, key: &str) -> Result<&'a str> {
    Ok(run[key]
        .as_str()
        .ok_or_else(|| format!("Run lacks {key}"))?)
}
fn set_state(simulator: &Arc<Mutex<Simulator>>, run: &Value, state: WorkflowState) -> Result {
    let mut simulator = simulator.lock().map_err(|_| "Simulator lock unavailable")?;
    let execution = simulator
        .executions
        .values_mut()
        .find(|execution| {
            execution.plan.attempt_id.as_str() == run["attempt_id"].as_str().unwrap_or_default()
        })
        .ok_or("Run has no simulated execution")?;
    execution.state = state;
    execution.sequence += 1;
    execution.remaining = None;
    Ok(())
}
fn advance(repository: &git2::Repository, number: usize) -> Result<String> {
    let root = repository
        .workdir()
        .ok_or("Example source has no worktree")?;
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(
        root.join("README.md"),
        format!(
            "# Example application\n\nSynthetic CI revision {number}. No source command is executed.\n"
        ),
    )?;
    std::fs::write(
        root.join("src/lib.rs"),
        format!("pub fn release_version() -> u32 {{ {number} }}\n"),
    )?;
    let mut index = repository.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    let tree_id = index.write_tree()?;
    let tree = repository.find_tree(tree_id)?;
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|id| repository.find_commit(id))
        .transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    let signature = git2::Signature::now("Synthetic contributor", "example@examples.invalid")?;
    let id = repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        &format!("Example change {number}: retain exact CI input"),
        &tree,
        &parents,
    )?;
    repository.set_head("refs/heads/main")?;
    Ok(id.to_string())
}
fn upload(engine: &mut Engine, run: &Value, failed: bool) -> Result {
    let id = exact(run, "id")?;
    for node in run["nodes"].as_array().ok_or("Run nodes absent")? {
        let node = node["id"]
            .as_str()
            .or_else(|| node["node_id"].as_str())
            .ok_or("Run node id absent")?;
        for (stream, content) in [
            (
                "stdout",
                format!(
                    "[synthetic example; no process executed]\nPreparing {node} on exact source {}\nstep 1/2: read admitted inputs\nstep 2/2: retain example output\n",
                    exact(run, "source_revision")?
                ),
            ),
            (
                "stderr",
                if failed && node == "test" {
                    "[synthetic example]\nerror: release_version_matches_manifest failed\nassertion failed at src/lib.rs:1: expected 2, observed 3\ntest result: FAILED. 2 passed; 1 failed; 1 ignored\n".into()
                } else {
                    "[synthetic example] No diagnostic error recorded.\n".into()
                },
            ),
        ] {
            let body = json!({"attempt_id":run["attempt_id"],"execution":run["execution"],"source_revision":run["source_revision"],"node_id":node,"stream":stream,"cursor":0,"body_base64":base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),"blake3":blake3::hash(content.as_bytes()).to_hex().to_string(),"truncated":false});
            call(engine, &format!("/api/runs/{id}/logs"), body, Role::Worker)?;
        }
    }
    let junit = if failed {
        b"<testsuite name='release' tests='4' failures='1' skipped='1'><testcase classname='release' name='identity_is_exact' time='0.02'/><testcase classname='release' name='artifact_is_bounded' time='0.01'/><testcase classname='release' name='release_version_matches_manifest' file='src/lib.rs' line='1' time='0.03'><failure message='expected 2, observed 3'>Synthetic regression: version mismatch</failure></testcase><testcase classname='release' name='remote_publication'><skipped message='No remote effects in examples'/></testcase></testsuite>".as_slice()
    } else {
        b"<testsuite name='release' tests='4' skipped='1'><testcase classname='release' name='identity_is_exact' time='0.02'/><testcase classname='release' name='artifact_is_bounded' time='0.01'/><testcase classname='release' name='release_version_matches_manifest' file='src/lib.rs' line='1' time='0.03'/><testcase classname='release' name='remote_publication'><skipped message='No remote effects in examples'/></testcase></testsuite>".as_slice()
    };
    let native = serde_json::to_vec(
        &json!({"schema":"kiln.example.evidence.v1","verdict":if failed {"block"} else {"pass"},"synthetic":true,"message":"Illustrative retained observation, not an executed check or acceptance","non_claims":["No source command executed","No merge or release authority"]}),
    )?;
    let review = serde_json::to_vec(&super::receipt::document(
        if failed { "rejected" } else { "accepted" },
        0,
    )?)?;
    let lcov = b"TN:synthetic-example\nSF:src/lib.rs\nDA:1,1\nDA:2,0\nLF:2\nLH:1\nend_of_record\n";
    for (kind, name, content) in [
        ("junit", "tests.xml", junit),
        ("lcov", "coverage.info", lcov.as_slice()),
        ("review", "review.json", review.as_slice()),
        ("gate", "gate.json", native.as_slice()),
        ("provenance", "provenance.json", native.as_slice()),
    ] {
        call(
            engine,
            &format!("/api/runs/{id}/reports"),
            json!({"run_id":id,"attempt_id":run["attempt_id"],"execution":run["execution"],"source_revision":run["source_revision"],"node_id":"test","kind":kind,"name":name,"body_base64":base64::engine::general_purpose::STANDARD.encode(content),"blake3":blake3::hash(content).to_hex().to_string()}),
            Role::Worker,
        )?;
    }
    let artifact =
        b"Synthetic release notes\nThis is a downloadable example artifact, not a built product.\n";
    call(
        engine,
        &format!("/api/runs/{id}/artifacts"),
        json!({"attempt_id":run["attempt_id"],"execution":run["execution"],"source_revision":run["source_revision"],"node_id":"test","name":"release-notes.txt","body_base64":base64::engine::general_purpose::STANDARD.encode(artifact),"blake3":blake3::hash(artifact).to_hex().to_string()}),
        Role::Worker,
    )?;
    let time = super::now_ms()?;
    call(
        engine,
        &format!("/api/runs/{id}/telemetry"),
        json!({"binding":{"run_id":id,"source_revision":run["source_revision"],"attempt_id":run["attempt_id"],"execution":run["execution"],"node_id":"test"},"sequence":1,"step":{"step_index":0,"started_at_ms":time.saturating_sub(45),"completed_at_ms":time,"duration_ms":45,"exit_code":if failed {101} else {0},"cpu_time_us":12000,"peak_memory_bytes":8_388_608}}),
        Role::Worker,
    )?;
    Ok(())
}

// r[impl kiln.web.exploration.scenario.coverage] r[impl kiln.web.exploration.scenario.local]
pub fn seed(root: &Path, port: u16) -> Result<Seed> {
    let repository = git2::Repository::init(root.join("ci-source"))?;
    advance(&repository, 1)?;
    let projects: Vec<_> = PROJECTS.iter().map(|(id,label)| json!({
        "id":id,"label":label,"repository_path":root.join("ci-source"),"reference":"refs/heads/main",
        "source_limits":{"max_entries":128,"max_path_bytes":1024,"max_file_bytes":65536,"max_total_bytes":262144},
        "workflow":{
            "socket_path":root.join(format!("{id}-simulator.sock")),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],
            "transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},
            "adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},
            "policy_id":"non-executing-example","provider_profile_id":format!("example-{id}"),"platform":"linux-x86_64",
            "nodes":NODE_IDS.iter().map(|node| json!({"id":node,"kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":blake3::hash(node.as_bytes()).to_hex().to_string()})).collect::<Vec<_>>(),
            "edges":[{"source":"build","target":"test"},{"source":"test","target":"package"}],
            "controls":{"cancel":true,"pause":false,"resume":false},
            "bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":16384,"max_total_log_bytes":262144}
        }
    })).collect();
    let profile: ServiceProfile = serde_json::from_value(
        json!({"schema":"kiln.web-service.v1","service_id":"synthetic-website-examples","listen_address":format!("127.0.0.1:{port}"),"origin":format!("https://127.0.0.1:{port}"),"tls":{"certificate_path":root.join("unused-cert.pem"),"private_key_path":root.join("unused-key.pem"),"min_version":"TLS1.3"},"operator_token_file":root.join("unused-operator.token"),"worker_token_file":root.join("unused-worker.token"),"state_directory":root.join("ci-state"),"projects":projects,"bounds":{"max_request_bytes":524288,"max_response_bytes":4194304,"max_header_bytes":16384,"max_connections":4,"timeout_seconds":3,"poll_interval_ms":1500,"max_projects":16,"max_runs":256,"max_artifact_bytes":65536,"max_artifacts":32,"max_run_artifact_bytes":524288,"max_total_artifact_bytes":8388608,"max_log_read_bytes":16384,"max_requests":4096,"max_events_per_run":128,"max_state_bytes":16777216}}),
    )?;
    let mut profile = profile;
    for project in &mut profile.projects {
        project.untrusted_capable = true;
    }
    let simulator = Arc::new(Mutex::new(Simulator::default()));
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    for project in &profile.projects {
        ports.insert(
            project.id.clone(),
            Box::new(Port {
                profile: project.execution_capabilities(profile.bounds.max_runs)?,
                simulator: Arc::clone(&simulator),
            }),
        );
    }
    let mut engine = Engine::open_with_ports(profile, ports)?;
    super::automation::configure(root, &mut engine)?;
    for project in ["checks", "release", "docs", "regression"] {
        call(
            &mut engine,
            "/api/projects",
            json!({"project_id":project,"request_id":format!("enroll-{project}")}),
            Role::Operator,
        )?;
    }
    let mut runs = BTreeMap::new();
    let mut examples = vec![
        Example::new(
            "Overview: failure first, health and retained activity",
            "/?page=overview",
            "Start with the failed project, then inspect active/queued work and target capacity. All execution is simulated.",
        ),
        Example::new(
            "CI projects and source-bound start",
            "/?page=projects",
            "Browse the admitted projects, choose a source revision and start an in-process simulated run. The sandbox begins unenrolled.",
        ),
        Example::new(
            "CI project history, streak and duration trend",
            "/?page=project&project=checks",
            "Inspect exact retained revisions and compare a failing run with its passing baseline.",
        ),
        Example::new(
            "Workers: busy, available, unknown and unenrolled",
            "/?page=workers",
            "Inspect configured simulated targets, project slots, exact current-run links and latest execution. Slots are not a physical machine census.",
        ),
    ];
    for (number, name, project, state) in [
        (2, "success", "checks", WorkflowState::Completed),
        (3, "failure", "checks", WorkflowState::Failed),
        (4, "docs", "docs", WorkflowState::Completed),
        (5, "cancelled", "checks", WorkflowState::Cancelled),
        (6, "unknown", "release", WorkflowState::Unknown),
        (7, "regression", "regression", WorkflowState::Failed),
    ] {
        advance(&repository, number)?;
        let run = call(
            &mut engine,
            "/api/runs",
            json!({"project_id":project,"request_id":format!("seed-{name}")}),
            Role::Operator,
        )?;
        upload(&mut engine, &run, state == WorkflowState::Failed)?;
        set_state(&simulator, &run, state)?;
        engine.poll()?;
        runs.insert(name.into(), exact(&run, "id")?.to_owned());
    }
    let failure_id = runs
        .get("failure")
        .ok_or("Failure scenario missing")?
        .clone();
    let partial = call(
        &mut engine,
        &format!("/api/runs/{failure_id}/rerun"),
        json!({"request_id":"seed-partial","mode":"failed"}),
        Role::Operator,
    )?;
    upload(&mut engine, &partial, false)?;
    set_state(&simulator, &partial, WorkflowState::Completed)?;
    engine.poll()?;
    runs.insert("partial".into(), exact(&partial, "id")?.to_owned());
    advance(&repository, 8)?;
    for name in ["running", "queued"] {
        let run = call(
            &mut engine,
            "/api/runs",
            json!({"project_id":"checks","request_id":format!("seed-{name}")}),
            Role::Operator,
        )?;
        runs.insert(name.into(), exact(&run, "id")?.to_owned());
    }
    let untrusted_revision = advance(&repository, 9)?;
    runs.insert(
        "awaiting-approval".into(),
        super::automation::awaiting_approval(&mut engine, &untrusted_revision)?,
    );
    let heartbeats = super::workers::configure(root, &mut engine)?;
    let success = runs.get("success").ok_or("Success scenario missing")?;
    for (tab, label, note) in [
        (
            "pipeline",
            "Pipeline graph and node details",
            "Select a node; inspect edges, progress, controls and recorded resource telemetry.",
        ),
        (
            "logs",
            "Logs: stdout/stderr, search and failure navigation",
            "Switch node/stream, search text and download retained logs. Every line is labeled synthetic.",
        ),
        (
            "artifacts",
            "Artifacts and authenticated byte downloads",
            "Download release-notes.txt or the original reports; bytes and BLAKE3 are retained locally.",
        ),
        (
            "tests",
            "Tests and coverage reports",
            "Inspect passed/failed/skipped cases, failure detail and LCOV coverage. Compare with the passing baseline.",
        ),
        (
            "reviews",
            "Run review, gate and provenance evidence",
            "Read retained native documents and their non-claims. A report grants no merge/release authority.",
        ),
        (
            "compare",
            "Compare runs: regression and source changes",
            "Compare with an exact passing run, inspect test changes, source and recorded timings.",
        ),
        (
            "activity",
            "Run activity and canonical events",
            "Inspect the command/event sequence for this exact run.",
        ),
        (
            "evidence",
            "Exact source, execution and evidence",
            "Inspect the bound source, candidate, workflow and artifact identities.",
        ),
    ] {
        examples.push(Example::new(
            label,
            format!("/?page=runs&run={failure_id}&view={tab}&other={success}"),
            note,
        ));
    }
    examples.push(Example::new("Automation: signed intake, interval/calendar schedules and withheld notifications","/?page=automation","Inspect the locally signed synthetic pull-request intake, edit admitted registrations, or enable a schedule. Execution stays in-process; notification reservations are intentionally never delivered."));
    for (name, id) in &runs {
        examples.push(Example::new(format!("Run state: {name}"),format!("/?page=runs&run={id}"),"Inspect this named synthetic scenario. Finished runs remain retained; running/queued/unknown stay explicit until local controls change them."));
    }
    simulator
        .lock()
        .map_err(|_| "Simulator lock unavailable")?
        .interactive = true;
    Ok(Seed {
        engine,
        runs,
        examples,
        heartbeats,
    })
}
