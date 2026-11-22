use super::*;

const TEST_MAX_ARTIFACTS: usize = 8;
const TEST_MAX_DEPENDENCIES: usize = 4;
const TEST_MAX_FILES: usize = 8;
const TEST_MAX_PATH_BYTES: usize = 256;
const TEST_MAX_ARTIFACT_BYTES: usize = 4096;
const TEST_MAX_TEMPLATE_BYTES: usize = 512;
const TEST_MAX_CONSTRAINTS: usize = 8;
const TEST_MAX_NON_CLAIMS: usize = 8;
const TEST_MAX_ISSUES: usize = 16;
const TEST_MAX_DIRECTORY_DEPTH: usize = 4;
const TEST_MAX_DIRECTORY_ENTRIES: usize = 16;
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn bounds() -> WorkflowBounds {
    WorkflowBounds {
        max_artifacts: TEST_MAX_ARTIFACTS,
        max_dependencies: TEST_MAX_DEPENDENCIES,
        max_files_per_artifact: TEST_MAX_FILES,
        max_path_bytes: TEST_MAX_PATH_BYTES,
        max_artifact_bytes: TEST_MAX_ARTIFACT_BYTES,
        max_template_bytes: TEST_MAX_TEMPLATE_BYTES,
        max_constraints: TEST_MAX_CONSTRAINTS,
        max_non_claims: TEST_MAX_NON_CLAIMS,
        max_issues: TEST_MAX_ISSUES,
        max_directory_depth: TEST_MAX_DIRECTORY_DEPTH,
        max_directory_entries: TEST_MAX_DIRECTORY_ENTRIES,
    }
}

fn artifact(
    id: &str,
    output_path: &str,
    requires: &[&str],
    structural_admission: &str,
) -> WorkflowArtifactPolicy {
    WorkflowArtifactPolicy {
        id: id.to_string(),
        generates: output_path.to_string(),
        output_path: output_path.to_string(),
        path_kind: WORKFLOW_PATH_KIND_FILE.to_string(),
        structural_admission: structural_admission.to_string(),
        requires: requires.iter().map(|value| (*value).to_string()).collect(),
        stage: "tasks".to_string(),
        instruction_template: "Write {{artifact}} for {{change}}.".to_string(),
        constraints: vec!["Keep the artifact deterministic.".to_string()],
        apply_prerequisites: vec!["gate:tasks".to_string()],
        skipped: false,
        skip_satisfies_dependencies: false,
    }
}

fn schema() -> WorkflowSchemaPolicy {
    WorkflowSchemaPolicy {
        schema_version: WORKFLOW_SCHEMA_VERSION_V1.to_string(),
        id: "spec-driven".to_string(),
        profile_version: 1,
        description: "test workflow".to_string(),
        spec_effect: WORKFLOW_SPEC_EFFECT_DELTA.to_string(),
        task_link_class: WORKFLOW_TASK_LINK_REQUIREMENT.to_string(),
        states: WORKFLOW_SUPPORTED_STATES.map(ToOwned::to_owned).to_vec(),
        bounds: bounds(),
        artifacts: vec![
            artifact("proposal", "proposal.md", &[], WORKFLOW_ADMISSION_PROPOSAL),
            artifact(
                "specs",
                "specs/spec/spec.md",
                &["proposal"],
                WORKFLOW_ADMISSION_DELTA_SPECS,
            ),
            artifact(
                "design",
                "design.md",
                &["proposal"],
                WORKFLOW_ADMISSION_DESIGN,
            ),
            artifact(
                "tasks",
                "tasks.md",
                &["specs", "design"],
                WORKFLOW_ADMISSION_TASKS,
            ),
        ],
        review_obligations: Vec::new(),
        receipt_fields: vec![
            "workflow_profile".to_string(),
            "workflow_profile_version".to_string(),
            "spec_effect".to_string(),
            "non_claims".to_string(),
        ],
        required_non_claims: vec![
            "workflow state proves structural facts only".to_string(),
            "workflow state does not prove release eligibility".to_string(),
        ],
    }
}

fn observed(id: &str, path: &str, hash: &str, valid: bool) -> WorkflowArtifactObservation {
    WorkflowArtifactObservation {
        artifact_id: id.to_string(),
        present: true,
        structurally_valid: valid,
        files: vec![WorkflowArtifactFile {
            path: path.to_string(),
            blake3: hash.to_string(),
        }],
        issue_codes: if valid {
            Vec::new()
        } else {
            vec![format!("{WORKFLOW_ISSUE_ARTIFACT_INVALID}:{id}")]
        },
    }
}

#[test]
fn valid_graph_derives_done_ready_and_blocked_states() {
    // r[verify cairn.artifact_workflow_contract.verification]
    // r[verify cairn.artifact_workflow_contract.status.ready]
    // r[verify cairn.artifact_workflow_contract.status.invalid]
    let schema = schema();
    admit_workflow_schemas(std::slice::from_ref(&schema), &["tasks".to_string()])
        .expect("schema admits");
    let report = derive_workflow_status(
        &schema,
        &[
            observed("proposal", "proposal.md", HASH_A, true),
            observed("specs", "specs/spec/spec.md", HASH_B, false),
        ],
    )
    .expect("status derives");

    assert_eq!(report.artifacts[0].state, WorkflowArtifactState::Done);
    assert_eq!(report.artifacts[1].state, WorkflowArtifactState::Blocked);
    assert_eq!(report.artifacts[2].state, WorkflowArtifactState::Ready);
    assert_eq!(report.artifacts[3].state, WorkflowArtifactState::Blocked);
    assert_eq!(report.next_actions, vec!["design"]);
}

#[test]
fn skipped_dependency_unlocks_only_when_policy_permits_it() {
    // r[verify cairn.artifact_workflow_contract.status.skipped]
    let mut schema = schema();
    let specs = schema
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.id == "specs")
        .expect("specs artifact");
    specs.skipped = true;
    specs.skip_satisfies_dependencies = true;
    let report = derive_workflow_status(
        &schema,
        &[
            observed("proposal", "proposal.md", HASH_A, true),
            observed("design", "design.md", HASH_B, true),
        ],
    )
    .expect("status derives");

    assert_eq!(report.artifacts[1].state, WorkflowArtifactState::Skipped);
    assert_eq!(report.artifacts[3].state, WorkflowArtifactState::Ready);
}

#[test]
fn ready_artifact_instructions_bind_upstream_files() {
    // r[verify cairn.artifact_workflow_contract.instructions.ready]
    let schema = schema();
    let report = derive_workflow_status(
        &schema,
        &[observed("proposal", "proposal.md", HASH_A, true)],
    )
    .expect("status derives");
    let instructions =
        build_workflow_instructions(&schema, &report, "design", ".cairn/changes/demo")
            .expect("instructions derive");

    assert_eq!(instructions.state, WorkflowArtifactState::Ready);
    assert_eq!(instructions.output_path, ".cairn/changes/demo/design.md");
    assert_eq!(instructions.upstream[0].files[0].blake3, HASH_A);
    assert!(instructions.instruction_template.contains("design"));
}

#[test]
fn invalid_graphs_fail_with_stable_diagnostics() {
    // r[verify cairn.artifact_workflow_contract.policy.invalid]
    let allowed = ["tasks".to_string()];

    let mut duplicate = schema();
    duplicate.artifacts[1].id = "proposal".to_string();
    assert_eq!(
        admit_workflow_schemas(&[duplicate], &allowed)
            .expect_err("duplicate fails")
            .code,
        WORKFLOW_ISSUE_POLICY_DUPLICATE_ARTIFACT
    );

    let mut unknown = schema();
    unknown.artifacts[1].requires = vec!["missing".to_string()];
    assert_eq!(
        admit_workflow_schemas(&[unknown], &allowed)
            .expect_err("unknown dependency fails")
            .code,
        WORKFLOW_ISSUE_POLICY_UNKNOWN_DEPENDENCY
    );

    let mut cycle = schema();
    cycle.artifacts[0].requires = vec!["tasks".to_string()];
    assert_eq!(
        admit_workflow_schemas(&[cycle], &allowed)
            .expect_err("cycle fails")
            .code,
        WORKFLOW_ISSUE_POLICY_CYCLE
    );

    let mut unsafe_path = schema();
    unsafe_path.artifacts[0].output_path = "../proposal.md".to_string();
    assert_eq!(
        admit_workflow_schemas(&[unsafe_path], &allowed)
            .expect_err("unsafe path fails")
            .code,
        WORKFLOW_ISSUE_POLICY_UNSAFE_PATH
    );

    let mut bad_template = schema();
    bad_template.artifacts[0].instruction_template = "Write {{unknown}}.".to_string();
    assert_eq!(
        admit_workflow_schemas(&[bad_template], &allowed)
            .expect_err("unknown token fails")
            .code,
        WORKFLOW_ISSUE_POLICY_TEMPLATE
    );
}

#[test]
fn outcome_machine_profile_is_required_by_runtime_policy_admission() {
    // r[verify cairn.outcome_machine_profile.selection]
    // r[verify cairn.outcome_machine_profile.verification]
    let profile_ids = [
        "spec-driven",
        "assurance",
        "no-spec-delta",
        "outcome-machine",
    ];
    let mut schemas = profile_ids
        .iter()
        .map(|profile_id| {
            let mut profile = schema();
            profile.id = (*profile_id).to_string();
            profile
        })
        .collect::<Vec<_>>();
    let outcome_machine = schemas
        .iter_mut()
        .find(|profile| profile.id == "outcome-machine")
        .expect("outcome-machine profile");
    let mut review_artifact = artifact(
        "outcome-machine",
        "outcome-machine.json",
        &["proposal"],
        WORKFLOW_ADMISSION_OUTCOME_MACHINE,
    );
    review_artifact.stage = "design".to_string();
    review_artifact.apply_prerequisites = vec!["gate:proposal".to_string()];
    outcome_machine.artifacts.push(review_artifact);
    for dependent_id in ["design", "tasks"] {
        outcome_machine
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.id == dependent_id)
            .expect("dependent artifact")
            .requires
            .push("outcome-machine".to_string());
    }
    let registry = WorkflowProfileRegistryPolicy {
        default_profile: "spec-driven".to_string(),
        allowed_profiles: profile_ids.map(ToOwned::to_owned).to_vec(),
        required_profiles_by_effect: Vec::new(),
        forbidden_selection_sources: [
            "user-global",
            "environment",
            "agent-prompt",
            "remote-package",
        ]
        .map(ToOwned::to_owned)
        .to_vec(),
    };
    admit_workflow_profile_registry(&registry, &schemas).expect("complete registry admits");

    let missing_outcome_machine = schemas
        .iter()
        .filter(|profile| profile.id != "outcome-machine")
        .cloned()
        .collect::<Vec<_>>();
    let error = admit_workflow_profile_registry(&registry, &missing_outcome_machine)
        .expect_err("missing outcome-machine profile fails");
    assert_eq!(error.code, WORKFLOW_ISSUE_PROFILE_REGISTRY);
    assert!(error.message.contains("outcome-machine"));

    let mut malformed = schemas;
    let review_artifact = malformed
        .iter_mut()
        .find(|profile| profile.id == "outcome-machine")
        .and_then(|profile| {
            profile
                .artifacts
                .iter_mut()
                .find(|artifact| artifact.id == "outcome-machine")
        })
        .expect("outcome-machine review artifact");
    review_artifact.stage = "tasks".to_string();
    let error = admit_workflow_profile_registry(&registry, &malformed)
        .expect_err("malformed outcome-machine profile fails");
    assert_eq!(error.code, WORKFLOW_ISSUE_PROFILE_REGISTRY);
    assert!(error.message.contains("design-stage"));
}

#[test]
fn unknown_and_inconsistent_observations_fail_closed() {
    // r[verify cairn.artifact_workflow_contract.instructions.unknown]
    let schema = schema();
    let unknown = WorkflowArtifactObservation::absent("unknown");
    assert_eq!(
        derive_workflow_status(&schema, &[unknown])
            .expect_err("unknown observation fails")
            .code,
        WORKFLOW_ISSUE_OBSERVATION_UNKNOWN
    );

    let inconsistent = WorkflowArtifactObservation {
        artifact_id: "proposal".to_string(),
        present: true,
        structurally_valid: true,
        files: Vec::new(),
        issue_codes: Vec::new(),
    };
    assert_eq!(
        derive_workflow_status(&schema, &[inconsistent])
            .expect_err("inconsistent observation fails")
            .code,
        WORKFLOW_ISSUE_OBSERVATION_INCONSISTENT
    );

    let report = derive_workflow_status(&schema, &[]).expect("empty observation set derives");
    assert_eq!(
        build_workflow_instructions(&schema, &report, "unknown", ".cairn/changes/demo")
            .expect_err("unknown selector fails")
            .code,
        WORKFLOW_ISSUE_ARTIFACT_UNKNOWN
    );
}
