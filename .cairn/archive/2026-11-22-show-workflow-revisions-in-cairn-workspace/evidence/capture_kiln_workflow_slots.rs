//! Capture `lattice.workflow-agent-revision-slots.v1` documents for Kiln #84.
//!
//! Drives one workflow-agent lifecycle through Lattice's own composition,
//! command dispatcher (`run_command_with_ports` with the fixture ports, as the
//! agent-revision suites do) and session seam, then reads each state back
//! with the `lattice agent-revision inspect` binary and writes its
//! `outcome.workflow_slots` member verbatim to
//! `$KILN_WORKFLOW_SLOTS_CAPTURE_DIR/<scenario>.json`.
//!
//! Inputs are the synthetic request and intent of
//! `tests/test_agent_revision_shell.rs`, so the captures are deterministic.

use lattice::agent_revision::Command;
use lattice::agent_revision::CommandArgs;
use lattice::agent_revision::FixturePreparation;
use lattice::agent_revision::FixtureQualification;
use lattice::agent_revision::Ports;
use lattice::agent_revision::SelectArgs;
use lattice::agent_revision::seam;
use lattice::implementation_attempt::cli::Request;
use lattice::store::Store;
use lattice_core::agent_revision::ports as verdict;

const FAMILY: &str = "rust-implementation";
const NAME: &str = "verify-change";

fn request(executable: char) -> Request {
    let envelope = lattice::implementation_attempt::ImplementationAttemptEnvelope {
        schema: lattice::implementation_attempt::IMPLEMENTATION_ATTEMPT_ENVELOPE_SCHEMA.into(),
        work_id: "work-25".into(),
        work_version: 1,
        source_kind: lattice::implementation_attempt::SourceKind::CairnTask,
        source_id: "task-25".into(),
        repository: "OnixResearch/lattice".into(),
        base: "a".repeat(40),
        readiness_snapshot: "b".repeat(64),
        task_spec_digest: "c".repeat(64),
        verification_profile: "f".repeat(64),
        attempt_id: "attempt-25".into(),
        fence: 7,
        workspace_id: "workspace-25".into(),
        worker_profile: "0".repeat(64),
        budget: lattice::implementation_attempt::AttemptBudget {
            max_turns: 1,
            max_seconds: 60,
            max_cost_micros: 1,
        },
        policy_digest: "d".repeat(64),
    };
    Request {
        schema: lattice::implementation_attempt::cli::REQUEST_SCHEMA.into(),
        admitted_envelope: envelope.identity(),
        envelope,
        expires_at_ms: u64::MAX,
        workspace_root: "/workspace".into(),
        read_paths: vec!["src".into()],
        write_paths: vec!["src".into()],
        program: "/usr/bin/worker".into(),
        sandbox_program: "/usr/bin/bwrap".into(),
        omp: lattice::candidate_worker::omp::Profile {
            revision: lattice::candidate_worker::omp::OMP_REVISION.into(),
            executable_blake3: executable.to_string().repeat(64),
            base_identity: "0".repeat(64),
            model: "lattice/model".into(),
            prompt: "make the candidate".into(),
            model_broker: lattice::candidate_worker::broker::Profile {
                schema: lattice::candidate_worker::broker::SCHEMA.into(),
                endpoint: "https://model.invalid/v1/chat/completions".into(),
                model: "model".into(),
                credential: Some(lattice::credentials::SecretRef::new("env", "MODEL_TOKEN", None)),
                relay: lattice::candidate_worker::broker::Executable {
                    path: "/nix/store/relay".into(),
                    blake3: "a".repeat(64),
                },
                shell: lattice::candidate_worker::broker::Executable {
                    path: "/nix/store/shell".into(),
                    blake3: "b".repeat(64),
                },
                sandbox_blake3: "c".repeat(64),
                expires_at_ms: u64::MAX,
                max_requests: 1,
            },
        },
        output_limit_bytes: 4096,
        allow_unchanged: false,
        worker_compute: lattice_core::execution_binding::request::ComputeRequirement::HostLocal {},
        execution_binding: lattice_core::execution_binding::request::Declaration {
            schema: lattice_core::execution_binding::request::DECLARATION_SCHEMA.into(),
            serving: None,
            compute: lattice_core::execution_binding::request::ComputeDeclaration::HostLocal {},
            model_routes: lattice_core::execution_binding::request::RouteDeclaration::NoModel {},
        },
        worker_capability: Some(lattice_core::execution_binding::capability::Declaration {
            schema: lattice_core::execution_binding::capability::CAPABILITY_SCHEMA.into(),
            definition: "rust-implementer".into(),
            task_families: Some(lattice_core::execution_binding::capability::NameSet::Declared {
                names: vec![FAMILY.into()],
            }),
            tools: Some(lattice_core::execution_binding::capability::NameSet::Declared {
                names: vec!["edit".into(), "read".into(), "write".into()],
            }),
        }),
        network: None,
        dispatch_plan: None,
        repair: None,
        lineage: None,
        workflow_agent: None,
    }
}

fn reference(digit: char) -> String {
    format!("blake3:{}", blake3::hash(digit.to_string().as_bytes()).to_hex())
}

fn intent_digest(value: &str) -> String {
    format!("b3:{}", blake3::hash(value.as_bytes()).to_hex())
}

/// The synthetic two-obligation intent of the agent-revision suite.
fn intent(guidance: &str) -> lattice::workflow::intent::CompileRequest {
    use lattice::workflow::intent as wi;
    let cohort = wi::SourceCohort {
        producer_identity: intent_digest("producer"),
        selection_identity: intent_digest("selection"),
        obligations_identity: intent_digest("obligations"),
        profile_identity: intent_digest("profile"),
        context_identity: intent_digest("context"),
        source_identity: intent_digest("source"),
        policy_identity: intent_digest("policy"),
        dependency_identity: intent_digest("dependencies"),
    };
    let obligation = |id: &str, class: &str, prerequisites: Vec<String>| wi::Obligation {
        id: id.into(),
        obligation_identity: intent_digest(&format!("obligation:{id}")),
        guidance_identity: intent_digest(&format!("{guidance}:{id}")),
        remaining_condition_identity: intent_digest(&format!("remaining:{id}")),
        precondition_identity: intent_digest(&format!("precondition:{id}")),
        operation_class: class.into(),
        action_identity: intent_digest(&format!("action:{id}")),
        prerequisites,
        required_observations: vec![wi::ObservationContract {
            id: format!("evidence-{id}"),
            schema: "lattice.workflow-intent-fixture-result.v1".into(),
            predicate_identity: intent_digest(&format!("predicate:{id}")),
        }],
        required_capabilities: std::collections::BTreeSet::from(["native-cairn".into()]),
    };
    wi::CompileRequest {
        intent: wi::Definition {
            schema: wi::INTENT_SCHEMA.into(),
            cohort: cohort.clone(),
            coverage: wi::ContextCoverage::Complete,
            unsupported_fields: vec![],
            obligations: vec![
                obligation("B", "review_wait", vec!["A".into()]),
                obligation("A", "cairn_validate", vec![]),
            ],
            alternatives: vec![],
            selected_alternative: None,
        },
        facts: wi::CompilerFacts {
            expected_cohort: cohort,
            compiler_artifact_identity: intent_digest("compiler"),
            runtime_profile_identity: intent_digest("runtime-profile"),
            available_capabilities: std::collections::BTreeSet::from(["native-cairn".into()]),
            available_implementations: std::collections::BTreeSet::new(),
        },
    }
}

fn run(command: Command, preparation: &mut FixturePreparation, qualification: &mut FixtureQualification) {
    lattice::agent_revision::run_command_with_ports(&CommandArgs { command }, Ports {
        preparation,
        qualification,
    })
    .expect("command");
}

fn answer(definition: &str, revision: &str, qualification: char) -> (verdict::Query, verdict::Verdict) {
    let query = verdict::Query {
        definition_identity: definition.into(),
        profile_revision: revision.into(),
        task_family: FAMILY.into(),
        required_class: verdict::EvidenceClass::TaskFamilyEvaluation,
        now_ms: 0,
    };
    let attachment = verdict::Attachment {
        qualification_ref: reference(qualification),
        subject: verdict::Subject {
            definition_identity: definition.into(),
            profile_revision: revision.into(),
            task_family: FAMILY.into(),
        },
        evidence_class: verdict::EvidenceClass::TaskFamilyEvaluation,
        protocol_identity: reference('a'),
        verifier_identity: reference('b'),
        observed_at_ms: 1_795_200_000_000,
        provenance: verdict::Provenance::Fixture,
    };
    (query, verdict::Verdict::Qualified(attachment))
}

fn select(store: &std::path::Path, definition: &str, revision: &str, qualification: char, expected: u64) -> SelectArgs {
    SelectArgs {
        store: store.into(),
        definition: definition.into(),
        revision: revision.into(),
        qualification_ref: reference(qualification),
        task_family: FAMILY.into(),
        required_class: "task_family_evaluation".into(),
        expected_generation: expected,
        request_id: format!("promote-{expected}"),
        qualification_attachments: Vec::new(),
    }
}

/// Record `composed` as the desired revision, as `desire-workflow` does after
/// composition.
fn desire(store_path: &std::path::Path, composed: &lattice::agent_revision::composite::Composed) {
    let store = Store::new(store_path).expect("store");
    lattice::agent_revision::transact(&store, &composed.definition, lattice::agent_revision::Seed::Create, |lifecycle| {
        lattice_core::agent_revision::ops::prepare::desire(lifecycle, &composed.bundle)
    })
    .expect("commits")
    .expect("desired");
}

/// Prepare the desired `composed` through the fixture preparation port and
/// promote it through the fixture qualification port.
fn prepare_promote(
    store_path: &std::path::Path,
    composed: &lattice::agent_revision::composite::Composed,
    artifact: char,
    qualification: char,
    expected: u64,
) {
    let prepared = verdict::Prepared::Prepared {
        artifacts: vec![reference(artifact)],
    };
    let mut preparation = FixturePreparation::new(vec![prepared], Vec::new());
    let mut unused = FixtureQualification::new(Vec::new());
    let prepare = Command::Prepare {
        store: store_path.into(),
        definition: composed.definition.clone(),
        revision: composed.revision.clone(),
    };
    run(prepare, &mut preparation, &mut unused);
    let answers = vec![answer(&composed.definition, &composed.revision, qualification)];
    let promote = Command::Promote(select(store_path, &composed.definition, &composed.revision, qualification, expected));
    run(promote, &mut FixturePreparation::default(), &mut FixtureQualification::new(answers));
}

/// The worker request naming one workflow-agent revision.
fn attempt(executable: char, composed: &lattice::agent_revision::composite::Composed) -> Request {
    let mut named = request(executable);
    named.workflow_agent = Some(lattice::implementation_attempt::WorkflowAgent {
        definition: composed.definition.clone(),
        revision: composed.revision.clone(),
    });
    named
}

/// `lattice agent-revision inspect`, keeping `outcome.workflow_slots`.
fn capture(directory: &std::path::Path, scenario: &str, store: &std::path::Path, definition: &str) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lattice"))
        .args(["agent-revision", "inspect", "--store"])
        .arg(store)
        .args(["--definition", definition])
        .output()
        .expect("lattice runs");
    assert!(output.status.success(), "inspect: {}", String::from_utf8_lossy(&output.stdout));
    let printed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("inspect prints JSON");
    let slots = &printed["outcome"]["workflow_slots"];
    assert_eq!(slots["schema"], "lattice.workflow-agent-revision-slots.v1");
    let mut bytes = serde_json::to_vec_pretty(slots).expect("encodes");
    bytes.push(b'\n');
    std::fs::write(directory.join(format!("{scenario}.json")), bytes).expect("capture written");
}

#[test]
fn capture_workflow_agent_revision_slots() {
    let Some(directory) = std::env::var_os("KILN_WORKFLOW_SLOTS_CAPTURE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("capture directory");
    let root = tempfile::tempdir().expect("scratch");
    let store_path = root.path().join("revisions.db");
    let worker = lattice::agent_revision::bundle::resolve(&request('e')).expect("resolves");
    let changed_worker = lattice::agent_revision::bundle::resolve(&request('9')).expect("resolves");
    let plan = lattice::workflow::intent::compile(intent("guidance")).expect("compiles");
    let guided = lattice::workflow::intent::compile(intent("guidance-2")).expect("compiles");
    let first = lattice::agent_revision::composite::compose(NAME, &plan, worker).expect("composes");
    let second = lattice::agent_revision::composite::compose(NAME, &guided, changed_worker).expect("composes");
    assert_eq!(first.definition, second.definition, "one workflow-agent definition");
    let definition = first.definition.clone();

    // 1. R1 desired only.
    desire(&store_path, &first);
    capture(&directory, "workflow-1-desired", &store_path, &definition);

    // 2. R1 prepared and promoted for new sessions.
    prepare_promote(&store_path, &first, '1', 'q', 0);
    capture(&directory, "workflow-2-promoted", &store_path, &definition);

    // 3. A session pinned to R1.
    let first_attempt = attempt('e', &first);
    let store = Store::new(&store_path).expect("store");
    let decision = seam::admit(&store, &first_attempt, "session-a").expect("seam");
    let first_admission = decision.admission().expect("admitted");
    assert_eq!(first_admission, seam::Admission::Managed);
    drop(store);
    capture(&directory, "workflow-3-session-pinned", &store_path, &definition);

    // 4. R2 (new guidance, changed worker revision) promoted; session-a stays
    //    on R1 and closes, session-b pins R2.
    desire(&store_path, &second);
    prepare_promote(&store_path, &second, '2', 'r', 1);
    let store = Store::new(&store_path).expect("store");
    let second_attempt = attempt('9', &second);
    let decision = seam::admit(&store, &second_attempt, "session-b").expect("seam");
    assert_eq!(decision.admission(), Some(seam::Admission::Managed));
    let closed = seam::close(&store, &first_attempt, "session-a", first_admission, &[reference('k')]).expect("seam");
    assert!(matches!(closed, seam::Closure::Closed { .. }), "{closed:?}");
    drop(store);
    capture(&directory, "workflow-4-second-revision", &store_path, &definition);
}
