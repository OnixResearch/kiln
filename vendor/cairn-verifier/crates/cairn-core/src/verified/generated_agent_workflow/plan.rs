use super::admission::issue;

pub const GENERATED_AGENT_WORKFLOW_PLAN_SCHEMA_V1: &str = "cairn.generated-agent-workflow-plan.v1";

// r[impl cairn.generated_agent_workflows.manifest]
// r[impl cairn.generated_agent_workflows.manifest.plan]
// r[impl cairn.generated_agent_workflows.manifest.freshness]
// r[impl cairn.generated_agent_workflows.install.obsolete]
// r[impl cairn.generated_agent_workflows.functional_boundary]
// r[impl cairn.generated_agent_workflows.functional_boundary.pure]
pub fn plan_generated_agent_workflow_install(
    catalog: &super::model::AdmittedGeneratedWorkflowCatalog,
    adapter_id: &str,
    destination: &str,
    policy_hash: &str,
    snapshot: &super::model::GeneratedWorkflowDestinationSnapshot,
) -> Result<super::model::GeneratedWorkflowInstallPlan, Vec<super::model::GeneratedWorkflowIssue>> {
    validate_destination(catalog, destination)?;
    let adapter = super::admission::find_generated_workflow_adapter(catalog, adapter_id)
        .map_err(|issue| vec![issue])?;
    let rendered = super::render::render_generated_agent_workflows(catalog, adapter_id)?;
    let observed = super::freshness::observation_map(&snapshot.files);
    let prior = super::freshness::supported_prior_manifest(snapshot, adapter_id, adapter.version);
    let prior_files = prior_file_map(prior.as_ref());
    let mut desired_paths = std::collections::BTreeSet::new();
    let mut recovery_required = snapshot.pending_transactions.clone();
    recovery_required.sort();
    recovery_required.dedup();
    let mut blockers = recovery_required
        .iter()
        .map(|path| {
            format!(
                "{}: {path}",
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_RECOVERY
            )
        })
        .collect::<Vec<_>>();
    let mut files = Vec::with_capacity(rendered.len());

    for file in rendered {
        let relative_path = super::render::join_generated_destination(
            destination,
            &file.relative_path,
            catalog.policy.bounds.max_path_bytes,
        )
        .map_err(|issue| vec![issue])?;
        desired_paths.insert(relative_path.clone());
        let observation = observed.get(relative_path.as_str());
        let expected_prior_blake3 = prior_files.get(&relative_path).cloned();
        let freshness = super::freshness::classify_file(
            observation,
            expected_prior_blake3.as_deref(),
            &file.content_blake3,
            prior.is_some(),
        );
        if super::freshness::file_blocks_install(&freshness, expected_prior_blake3.is_some()) {
            blockers.push(conflict(&relative_path));
        }
        files.push(super::model::GeneratedWorkflowPlannedFile {
            workflow_id: file.workflow_id,
            relative_path,
            generated_blake3: file.content_blake3,
            descriptor_identity: file.descriptor_identity,
            expected_prior_blake3,
            observed_blake3: observation.and_then(|value| value.content_blake3.clone()),
            freshness,
            contents: file.contents,
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let removal_candidates = removal_candidates(prior.as_ref(), &desired_paths);
    let manifest_path = super::render::join_generated_destination(
        destination,
        &super::render::generated_manifest_relative_path(adapter_id),
        catalog.policy.bounds.max_path_bytes,
    )
    .map_err(|issue| vec![issue])?;
    let mut manifest = build_manifest(
        catalog,
        adapter.id.as_str(),
        adapter.version,
        destination,
        policy_hash,
        &files,
        &removal_candidates,
    );
    manifest.manifest_identity = super::freshness::generated_workflow_manifest_identity(&manifest);
    let manifest_bytes = serialize_manifest(&manifest)?;
    let manifest_freshness =
        super::freshness::classify_manifest(snapshot, prior.as_ref(), &manifest);
    if matches!(
        manifest_freshness,
        super::model::GeneratedWorkflowFreshness::UnsupportedVersion
            | super::model::GeneratedWorkflowFreshness::Modified
            | super::model::GeneratedWorkflowFreshness::UnmanagedConflict
            | super::model::GeneratedWorkflowFreshness::SymlinkConflict
    ) {
        blockers.push(conflict(&manifest_path));
    }
    blockers.sort();
    blockers.dedup();
    let mut plan = super::model::GeneratedWorkflowInstallPlan {
        schema: GENERATED_AGENT_WORKFLOW_PLAN_SCHEMA_V1.to_string(),
        adapter_id: adapter.id.clone(),
        adapter_version: adapter.version,
        destination: destination.to_string(),
        policy_hash: policy_hash.to_string(),
        catalog_identity: catalog.catalog_identity.clone(),
        max_file_bytes: catalog.policy.bounds.max_file_bytes,
        files,
        manifest_path,
        manifest,
        manifest_bytes,
        expected_prior_manifest_identity: prior
            .as_ref()
            .map(|manifest| manifest.manifest_identity.clone()),
        manifest_freshness,
        removal_candidates,
        recovery_required,
        blockers,
        plan_hash: String::new(),
        non_claims: catalog.policy.required_non_claims.clone(),
    };
    plan.plan_hash = generated_workflow_plan_identity(&plan);
    Ok(plan)
}

pub fn generated_workflow_plan_identity(
    plan: &super::model::GeneratedWorkflowInstallPlan,
) -> String {
    crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "schema": plan.schema,
        "adapter_id": plan.adapter_id,
        "adapter_version": plan.adapter_version,
        "destination": plan.destination,
        "policy_hash": plan.policy_hash,
        "catalog_identity": plan.catalog_identity,
        "max_file_bytes": plan.max_file_bytes,
        "files": plan.files.iter().map(|file| serde_json::json!({
            "workflow_id": file.workflow_id,
            "relative_path": file.relative_path,
            "generated_blake3": file.generated_blake3,
            "descriptor_identity": file.descriptor_identity,
            "expected_prior_blake3": file.expected_prior_blake3,
            "observed_blake3": file.observed_blake3,
            "freshness": file.freshness,
        })).collect::<Vec<_>>(),
        "manifest_path": plan.manifest_path,
        "manifest": plan.manifest,
        "expected_prior_manifest_identity": plan.expected_prior_manifest_identity,
        "manifest_freshness": plan.manifest_freshness,
        "removal_candidates": plan.removal_candidates,
        "recovery_required": plan.recovery_required,
        "blockers": plan.blockers,
        "non_claims": plan.non_claims,
    }))
}

fn validate_destination(
    catalog: &super::model::AdmittedGeneratedWorkflowCatalog,
    destination: &str,
) -> Result<(), Vec<super::model::GeneratedWorkflowIssue>> {
    if super::render::safe_generated_destination_path(
        destination,
        catalog.policy.bounds.max_path_bytes,
    ) {
        return Ok(());
    }
    Err(vec![issue(
        super::model::GENERATED_AGENT_WORKFLOW_ISSUE_DESTINATION,
        "destination",
        "destination must be an explicit safe project-relative path",
    )])
}

fn prior_file_map(
    prior: Option<&super::model::GeneratedWorkflowManifest>,
) -> std::collections::BTreeMap<String, String> {
    prior
        .map(|manifest| {
            manifest
                .files
                .iter()
                .map(|file| (file.relative_path.clone(), file.generated_blake3.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn removal_candidates(
    prior: Option<&super::model::GeneratedWorkflowManifest>,
    desired_paths: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    let Some(manifest) = prior else {
        return Vec::new();
    };
    let mut candidates = manifest
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .chain(manifest.removal_candidates.iter().cloned())
        .filter(|path| !desired_paths.contains(path))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn build_manifest(
    catalog: &super::model::AdmittedGeneratedWorkflowCatalog,
    adapter_id: &str,
    adapter_version: u32,
    destination: &str,
    policy_hash: &str,
    files: &[super::model::GeneratedWorkflowPlannedFile],
    removal_candidates: &[String],
) -> super::model::GeneratedWorkflowManifest {
    super::model::GeneratedWorkflowManifest {
        schema: super::model::GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1.to_string(),
        generator_version: super::model::GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION.to_string(),
        policy_hash: policy_hash.to_string(),
        catalog_identity: catalog.catalog_identity.clone(),
        adapter_id: adapter_id.to_string(),
        adapter_version,
        destination: destination.to_string(),
        files: files
            .iter()
            .map(|file| super::model::GeneratedWorkflowManifestFile {
                workflow_id: file.workflow_id.clone(),
                relative_path: file.relative_path.clone(),
                generated_blake3: file.generated_blake3.clone(),
                expected_prior_blake3: Some(file.generated_blake3.clone()),
                descriptor_identity: file.descriptor_identity.clone(),
            })
            .collect(),
        removal_candidates: removal_candidates.to_vec(),
        non_claims: catalog.policy.required_non_claims.clone(),
        manifest_identity: String::new(),
    }
}

fn serialize_manifest(
    manifest: &super::model::GeneratedWorkflowManifest,
) -> Result<String, Vec<super::model::GeneratedWorkflowIssue>> {
    let value = serde_json::to_value(manifest).map_err(|error| {
        vec![issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_POLICY,
            "manifest",
            format!("manifest serialization failed: {error}"),
        )]
    })?;
    Ok(format!(
        "{}\n",
        crate::verified::evidence::canonical_json(&value)
    ))
}

fn conflict(path: &str) -> String {
    format!(
        "{}: {path}",
        super::model::GENERATED_AGENT_WORKFLOW_ISSUE_CONFLICT
    )
}
