#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStage {
    Proposal,
    Design,
    Tasks,
}

impl GateStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Design => "design",
            Self::Tasks => "tasks",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateIssue {
    pub code: String,
    pub message: String,
    pub scope: String,
    pub promoted: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    pub change: String,
    pub stage: String,
    pub verdict: String,
    pub issues: Vec<GateIssue>,
    pub substance: Vec<super::substance::ArtifactSubstanceInventory>,
    pub task_ordering: Option<super::task_ordering::TaskOrderingGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateArtifact {
    pub path: String,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceStrengthSummary {
    pub docs_only: usize,
    pub fixture: usize,
    pub probe: usize,
    pub property: usize,
    pub model: usize,
    pub formal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateSnapshot {
    pub change: crate::verified::repo::ChangeStatus,
    pub proposal: Option<GateArtifact>,
    pub design: Option<GateArtifact>,
    pub tasks: Option<GateArtifact>,
    pub delta_specs: Vec<GateArtifact>,
}

impl GateSnapshot {
    pub fn from_change(change: crate::verified::repo::ChangeStatus) -> Self {
        Self {
            change,
            proposal: None,
            design: None,
            tasks: None,
            delta_specs: Vec::new(),
        }
    }
}

pub fn evaluate_gate(
    change: &crate::verified::repo::ChangeStatus,
    stage: GateStage,
    policy: &crate::verified::policy::Policy,
) -> GateReport {
    evaluate_gate_snapshot(&GateSnapshot::from_change(change.clone()), stage, policy)
}

pub fn classify_gate_evidence(snapshot: &GateSnapshot) -> EvidenceStrengthSummary {
    // r[impl cairn.structured_lifecycle.probes]
    // r[impl cairn.structured_lifecycle.migration]
    let mut summary = EvidenceStrengthSummary {
        docs_only: 0,
        fixture: 0,
        probe: 0,
        property: 0,
        model: 0,
        formal: 0,
    };
    for marker in gate_markdown_blobs(snapshot)
        .flat_map(|markdown| markdown.lines())
        .filter_map(verify_marker_text)
    {
        classify_verify_marker(&mut summary, marker);
    }
    summary
}

pub fn evaluate_gate_snapshot(
    snapshot: &GateSnapshot,
    stage: GateStage,
    policy: &crate::verified::policy::Policy,
) -> GateReport {
    // r[impl cairn.supersede.gate_policy]
    // r[impl cairn.task_ordering.gate]
    // r[impl cairn.supersede.gate_policy.references]
    // r[impl cairn.supersede.gate_policy.task_links]
    // r[impl cairn.supersede.gate_policy.exemptions]
    let change = &snapshot.change;
    let mut issues = Vec::with_capacity(4);
    if !policy
        .gate_stages
        .iter()
        .any(|candidate| candidate == stage.as_str())
    {
        issues.push(gate_issue(
            format!("stage not enabled by policy: {}", stage.as_str()),
            policy,
        ));
    }
    issues.extend(required_artifact_issues(change, stage, policy));
    let task_ordering = if matches!(stage, GateStage::Tasks) {
        snapshot.tasks.as_ref().map(|tasks| {
            let graph = super::task_ordering::build_task_ordering_graph(&tasks.markdown, policy);
            issues.extend(
                graph
                    .issues
                    .iter()
                    .map(|issue| task_ordering_gate_issue(&tasks.path, issue, policy)),
            );
            graph
        })
    } else {
        None
    };
    if matches!(stage, GateStage::Tasks) {
        issues.extend(
            super::changes::change_issues(change, policy)
                .into_iter()
                .map(|message| gate_issue(message, policy)),
        );
    }
    let (substance_issues, substance) =
        super::substance::artifact_substance_issues(snapshot, stage, policy);
    issues.extend(substance_issues);
    issues.extend(super::references::reference_hygiene_issues(
        snapshot, stage, policy,
    ));
    let verdict = if issues.is_empty() { "PASS" } else { "FAIL" }.to_string();
    GateReport {
        change: change.name.clone(),
        stage: stage.as_str().to_string(),
        verdict,
        issues,
        substance,
        task_ordering,
    }
}

pub fn evaluate_profile_gate_snapshot(
    snapshot: &GateSnapshot,
    stage: GateStage,
    policy: &crate::verified::policy::Policy,
    profile: &crate::verified::artifact_workflow::WorkflowSchemaPolicy,
    evaluation: &crate::verified::artifact_workflow::WorkflowProfileEvaluation,
    workflow_status: &crate::verified::artifact_workflow::WorkflowStatusReport,
) -> GateReport {
    // r[impl cairn.lifecycle_workflow_profiles.integration]
    // r[impl cairn.lifecycle_workflow_profiles.no_spec]
    let adjusted = profile_gate_snapshot(snapshot, profile, evaluation);
    let mut report = evaluate_gate_snapshot(&adjusted, stage, policy);
    for profile_issue in evaluation.issues.iter().filter(|issue| {
        crate::verified::artifact_workflow::profile_issue_applies_to_stage(issue, stage.as_str())
    }) {
        report.issues.push(GateIssue {
            code: profile_issue.code.clone(),
            message: profile_issue.message.clone(),
            scope: policy.gate_finding_scope.clone(),
            promoted: policy.gate_finding_promoted.clone(),
        });
    }
    for status in workflow_status.artifacts.iter().filter(|status| {
        let Some(artifact) = profile
            .artifacts
            .iter()
            .find(|artifact| artifact.id == status.id)
        else {
            return false;
        };
        profile_artifact_is_gate_specific(artifact.structural_admission.as_str())
            && artifact_stage_applies(&artifact.stage, stage)
            && status.state != crate::verified::artifact_workflow::WorkflowArtifactState::Done
    }) {
        report.issues.push(GateIssue {
            code: "cairn.workflow.profile.artifact".to_string(),
            message: format!(
                "workflow profile artifact is not structurally complete: {} ({})",
                status.id,
                status.state.as_str()
            ),
            scope: policy.gate_finding_scope.clone(),
            promoted: policy.gate_finding_promoted.clone(),
        });
    }
    report.issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.message.cmp(&right.message))
    });
    report
        .issues
        .dedup_by(|left, right| left.code == right.code && left.message == right.message);
    report.verdict = if report.issues.is_empty() {
        "PASS"
    } else {
        "FAIL"
    }
    .to_string();
    report
}

fn profile_gate_snapshot(
    snapshot: &GateSnapshot,
    profile: &crate::verified::artifact_workflow::WorkflowSchemaPolicy,
    evaluation: &crate::verified::artifact_workflow::WorkflowProfileEvaluation,
) -> GateSnapshot {
    if profile.spec_effect != crate::verified::artifact_workflow::WORKFLOW_SPEC_EFFECT_NONE {
        return snapshot.clone();
    }
    let mut adjusted = snapshot.clone();
    let synthetic_path = format!("{}/acceptance.md#profile-adapter", adjusted.change.path);
    adjusted.change.delta_specs = vec![synthetic_path.clone()];
    adjusted.delta_specs = vec![GateArtifact {
        path: synthetic_path,
        markdown: crate::verified::artifact_workflow::synthetic_acceptance_spec(
            &evaluation.acceptance_ids,
        ),
    }];
    if let Some(tasks) = &mut adjusted.tasks {
        tasks.markdown =
            crate::verified::artifact_workflow::tasks_with_requirement_links(&tasks.markdown);
    }
    adjusted
}

fn profile_artifact_is_gate_specific(admission: &str) -> bool {
    !matches!(
        admission,
        crate::verified::artifact_workflow::WORKFLOW_ADMISSION_PROPOSAL
            | crate::verified::artifact_workflow::WORKFLOW_ADMISSION_DELTA_SPECS
            | crate::verified::artifact_workflow::WORKFLOW_ADMISSION_DESIGN
            | crate::verified::artifact_workflow::WORKFLOW_ADMISSION_TASKS
    )
}

fn artifact_stage_applies(artifact_stage: &str, gate_stage: GateStage) -> bool {
    match gate_stage {
        GateStage::Proposal => artifact_stage == GateStage::Proposal.as_str(),
        GateStage::Design => matches!(artifact_stage, "proposal" | "design"),
        GateStage::Tasks => matches!(artifact_stage, "proposal" | "design" | "tasks"),
    }
}

fn required_artifact_issues(
    change: &crate::verified::repo::ChangeStatus,
    stage: GateStage,
    policy: &crate::verified::policy::Policy,
) -> Vec<GateIssue> {
    let mut issues = Vec::with_capacity(3);
    if !change.has_proposal {
        issues.push(gate_issue(
            format!("{}: missing proposal.md", change.name),
            policy,
        ));
    }
    if matches!(stage, GateStage::Design | GateStage::Tasks) && change.delta_specs.is_empty() {
        issues.push(gate_issue(
            format!("{}: missing delta specs", change.name),
            policy,
        ));
    }
    if matches!(stage, GateStage::Design | GateStage::Tasks) && !change.has_design {
        issues.push(gate_issue(
            format!("{}: missing design.md", change.name),
            policy,
        ));
    }
    if matches!(stage, GateStage::Tasks) && !change.has_tasks {
        issues.push(gate_issue(
            format!("{}: missing tasks.md", change.name),
            policy,
        ));
    }
    issues
}

pub fn finalize_gate_report(
    mut report: GateReport,
    mut external_issues: Vec<GateIssue>,
) -> GateReport {
    // r[impl cairn.probe_execution_boundary.authoritative_gate_composition]
    // r[impl cairn.probe_execution_boundary.authoritative_gate_composition.passing]
    // r[impl cairn.probe_execution_boundary.authoritative_gate_composition.shell_finding]
    external_issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.message.cmp(&right.message))
    });
    external_issues
        .dedup_by(|left, right| left.code == right.code && left.message == right.message);
    report.issues.extend(external_issues);
    report.verdict = if report.issues.is_empty() {
        "PASS"
    } else {
        "FAIL"
    }
    .to_string();
    assert_eq!(report.verdict == "PASS", report.issues.is_empty());
    report
}

pub fn validation_issue_code(message: &str) -> String {
    const LIFECYCLE_DIAGNOSTIC_PREFIX: &str = "lifecycle_validation.";
    const GENERIC_GATE_ISSUE_CODE: &str = "cairn.gate.validation";
    let Some(start) = message.find(LIFECYCLE_DIAGNOSTIC_PREFIX) else {
        return GENERIC_GATE_ISSUE_CODE.to_string();
    };
    message[start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        .collect()
}

pub(super) fn gate_issue(message: String, policy: &crate::verified::policy::Policy) -> GateIssue {
    GateIssue {
        code: validation_issue_code(&message),
        message,
        scope: policy.gate_finding_scope.clone(),
        promoted: policy.gate_finding_promoted.clone(),
    }
}

fn task_ordering_gate_issue(
    path: &str,
    issue: &super::task_ordering::TaskOrderingIssue,
    policy: &crate::verified::policy::Policy,
) -> GateIssue {
    let location = issue
        .line
        .map_or_else(|| path.to_string(), |line| format!("{path}:{line}"));
    GateIssue {
        code: issue.code.clone(),
        message: format!("{location}: {}", issue.message),
        scope: policy.gate_finding_scope.clone(),
        promoted: policy.gate_finding_promoted.clone(),
    }
}

fn gate_markdown_blobs(snapshot: &GateSnapshot) -> impl Iterator<Item = &str> {
    snapshot
        .proposal
        .iter()
        .chain(snapshot.design.iter())
        .chain(snapshot.tasks.iter())
        .chain(snapshot.delta_specs.iter())
        .map(|artifact| artifact.markdown.as_str())
}

fn verify_marker_text(line: &str) -> Option<&str> {
    let marker_start = line.find("r[verify ")? + "r[verify ".len();
    let marker_end = line[marker_start..].find(']')? + marker_start;
    Some(&line[marker_start..marker_end])
}

fn classify_verify_marker(summary: &mut EvidenceStrengthSummary, marker: &str) {
    if marker.contains("formal") || marker.contains("proof") || marker.contains("verus") {
        summary.formal += 1;
    } else if marker.contains("model") {
        summary.model += 1;
    } else if marker.contains("property") {
        summary.property += 1;
    } else if marker.contains("probe") {
        summary.probe += 1;
    } else if marker.contains("fixture") {
        summary.fixture += 1;
    } else {
        summary.docs_only += 1;
    }
}
