use super::model::*;

const WORKFLOW_PLACEHOLDER_TOKEN: &str = "CAIRN_PLACEHOLDER";

#[must_use]
pub fn workflow_markdown_is_structurally_present(markdown: &str) -> bool {
    !markdown.contains(WORKFLOW_PLACEHOLDER_TOKEN)
        && markdown
            .lines()
            .map(str::trim)
            .any(|line| !line.is_empty() && line.bytes().any(|byte| byte.is_ascii_alphanumeric()))
}

// r[impl cairn.artifact_workflow_contract.status]
// r[impl cairn.artifact_workflow_contract.boundary]
pub fn derive_workflow_status(
    schema: &WorkflowSchemaPolicy,
    observations: &[WorkflowArtifactObservation],
) -> Result<WorkflowStatusReport, WorkflowAdmissionError> {
    let order = super::admission::workflow_topological_order(schema)?;
    let observations = normalize_observations(schema, observations)?;
    let mut statuses = std::collections::BTreeMap::new();
    let mut issues = Vec::new();

    for artifact_id in &order {
        let artifact = schema
            .artifacts
            .iter()
            .find(|candidate| candidate.id == *artifact_id)
            .ok_or_else(|| unknown_artifact_error(schema, artifact_id))?;
        let observation = observations
            .get(artifact_id)
            .cloned()
            .unwrap_or_else(|| WorkflowArtifactObservation::absent(artifact_id));
        let status = derive_artifact_status(schema, artifact, &observation, &statuses);
        issues.extend(status.blockers.iter().filter_map(|blocker| {
            if blocker.starts_with(WORKFLOW_ISSUE_ARTIFACT_INVALID) {
                Some(blocker.clone())
            } else {
                None
            }
        }));
        let replaced = statuses.insert(artifact_id.clone(), status);
        assert!(replaced.is_none());
    }

    let artifacts = order
        .iter()
        .filter_map(|artifact_id| statuses.get(artifact_id).cloned())
        .map(|mut status| {
            status.unlocks = direct_unlocks(schema, &status.id, &order);
            status
        })
        .collect::<Vec<_>>();
    let next_actions = artifacts
        .iter()
        .filter(|artifact| artifact.state == WorkflowArtifactState::Ready)
        .map(|artifact| artifact.id.clone())
        .collect::<Vec<_>>();
    issues.sort();
    issues.dedup();
    assert!(issues.len() <= schema.bounds.max_issues);

    Ok(WorkflowStatusReport {
        schema_version: WORKFLOW_STATUS_SCHEMA_V1.to_string(),
        schema_id: schema.id.clone(),
        schema_description: schema.description.clone(),
        valid: true,
        artifacts,
        next_actions,
        issues,
        non_claims: schema.required_non_claims.clone(),
    })
}

// r[impl cairn.artifact_workflow_contract.instructions]
pub fn build_workflow_instructions(
    schema: &WorkflowSchemaPolicy,
    report: &WorkflowStatusReport,
    artifact_id: &str,
    change_path: &str,
) -> Result<WorkflowInstructionEnvelope, WorkflowAdmissionError> {
    let artifact = schema
        .artifacts
        .iter()
        .find(|candidate| candidate.id == artifact_id)
        .ok_or_else(|| unknown_artifact_error(schema, artifact_id))?;
    let status = report
        .artifacts
        .iter()
        .find(|candidate| candidate.id == artifact_id)
        .ok_or_else(|| unknown_artifact_error(schema, artifact_id))?;
    let upstream = artifact
        .requires
        .iter()
        .filter_map(|required| {
            report
                .artifacts
                .iter()
                .find(|candidate| candidate.id == *required)
                .map(|candidate| WorkflowUpstreamArtifact {
                    artifact_id: candidate.id.clone(),
                    state: candidate.state,
                    files: candidate.files.clone(),
                })
        })
        .collect::<Vec<_>>();
    let output_path = format!("{change_path}/{}", artifact.output_path);

    Ok(WorkflowInstructionEnvelope {
        schema_version: WORKFLOW_INSTRUCTIONS_SCHEMA_V1.to_string(),
        schema_id: schema.id.clone(),
        artifact_id: artifact.id.clone(),
        state: status.state,
        output_path,
        instruction_template: render_template(
            &artifact.instruction_template,
            artifact_id,
            change_path,
        ),
        constraints: artifact.constraints.clone(),
        requires: artifact.requires.clone(),
        apply_prerequisites: artifact.apply_prerequisites.clone(),
        blockers: status.blockers.clone(),
        upstream,
        non_claims: schema.required_non_claims.clone(),
    })
}

fn normalize_observations(
    schema: &WorkflowSchemaPolicy,
    observations: &[WorkflowArtifactObservation],
) -> Result<std::collections::BTreeMap<String, WorkflowArtifactObservation>, WorkflowAdmissionError>
{
    let known = schema
        .artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut normalized = std::collections::BTreeMap::new();
    for observation in observations {
        if !known.contains(observation.artifact_id.as_str()) {
            return Err(WorkflowAdmissionError::new(
                WORKFLOW_ISSUE_OBSERVATION_UNKNOWN,
                format!("observations.{}", observation.artifact_id),
                "observation references an unknown workflow artifact",
            ));
        }
        let absent_with_files = !observation.present && !observation.files.is_empty();
        let valid_without_files =
            observation.present && observation.structurally_valid && observation.files.is_empty();
        if absent_with_files || valid_without_files {
            return Err(WorkflowAdmissionError::new(
                WORKFLOW_ISSUE_OBSERVATION_INCONSISTENT,
                format!("observations.{}", observation.artifact_id),
                "artifact presence and validity must match the bounded file inventory",
            ));
        }
        let mut observation = observation.clone();
        observation
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        observation.issue_codes.sort();
        observation.issue_codes.dedup();
        if normalized
            .insert(observation.artifact_id.clone(), observation)
            .is_some()
        {
            return Err(WorkflowAdmissionError::new(
                WORKFLOW_ISSUE_OBSERVATION_DUPLICATE,
                "observations",
                "workflow artifact observations must be unique",
            ));
        }
    }
    Ok(normalized)
}

fn derive_artifact_status(
    schema: &WorkflowSchemaPolicy,
    artifact: &WorkflowArtifactPolicy,
    observation: &WorkflowArtifactObservation,
    prior: &std::collections::BTreeMap<String, WorkflowArtifactStatus>,
) -> WorkflowArtifactStatus {
    let missing = missing_dependencies(schema, artifact, prior);
    let mut blockers = dependency_blockers(&missing);
    let state = if artifact.skipped {
        WorkflowArtifactState::Skipped
    } else if observation.present && !observation.structurally_valid {
        append_observation_blockers(&mut blockers, artifact, observation);
        WorkflowArtifactState::Blocked
    } else if observation.present {
        WorkflowArtifactState::Done
    } else if missing.is_empty() {
        WorkflowArtifactState::Ready
    } else {
        WorkflowArtifactState::Blocked
    };
    blockers.sort();
    blockers.dedup();

    WorkflowArtifactStatus {
        id: artifact.id.clone(),
        state,
        output_path: artifact.output_path.clone(),
        path_kind: artifact.path_kind.clone(),
        structural_admission: artifact.structural_admission.clone(),
        stage: artifact.stage.clone(),
        requires: artifact.requires.clone(),
        missing,
        unlocks: Vec::new(),
        blockers,
        files: observation.files.clone(),
    }
}

fn missing_dependencies(
    schema: &WorkflowSchemaPolicy,
    artifact: &WorkflowArtifactPolicy,
    prior: &std::collections::BTreeMap<String, WorkflowArtifactStatus>,
) -> Vec<String> {
    artifact
        .requires
        .iter()
        .filter(|required| {
            let Some(status) = prior.get(*required) else {
                return true;
            };
            match status.state {
                WorkflowArtifactState::Done => false,
                WorkflowArtifactState::Skipped => schema
                    .artifacts
                    .iter()
                    .find(|candidate| candidate.id.as_str() == required.as_str())
                    .is_none_or(|candidate| !candidate.skip_satisfies_dependencies),
                WorkflowArtifactState::Ready | WorkflowArtifactState::Blocked => true,
            }
        })
        .cloned()
        .collect()
}

fn dependency_blockers(missing: &[String]) -> Vec<String> {
    missing
        .iter()
        .map(|required| format!("{WORKFLOW_ISSUE_DEPENDENCY_BLOCKED}:{required}"))
        .collect()
}

fn append_observation_blockers(
    blockers: &mut Vec<String>,
    artifact: &WorkflowArtifactPolicy,
    observation: &WorkflowArtifactObservation,
) {
    if observation.issue_codes.is_empty() {
        blockers.push(format!("{WORKFLOW_ISSUE_ARTIFACT_INVALID}:{}", artifact.id));
    } else {
        blockers.extend(observation.issue_codes.iter().cloned());
    }
}

fn direct_unlocks(
    schema: &WorkflowSchemaPolicy,
    artifact_id: &str,
    order: &[String],
) -> Vec<String> {
    order
        .iter()
        .filter(|candidate_id| {
            schema
                .artifacts
                .iter()
                .find(|artifact| artifact.id.as_str() == candidate_id.as_str())
                .is_some_and(|artifact| {
                    artifact
                        .requires
                        .iter()
                        .any(|required| required == artifact_id)
                })
        })
        .cloned()
        .collect()
}

fn render_template(template: &str, artifact_id: &str, change_path: &str) -> String {
    template
        .replace("{{artifact}}", artifact_id)
        .replace("{{change}}", change_path)
}

fn unknown_artifact_error(
    schema: &WorkflowSchemaPolicy,
    artifact_id: &str,
) -> WorkflowAdmissionError {
    let known = schema
        .artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    WorkflowAdmissionError::new(
        WORKFLOW_ISSUE_ARTIFACT_UNKNOWN,
        format!("schemas.{}.artifacts", schema.id),
        format!("unknown artifact {artifact_id}; known artifacts: {known}"),
    )
}
