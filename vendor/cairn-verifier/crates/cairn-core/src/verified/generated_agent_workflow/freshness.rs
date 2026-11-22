pub(super) fn observation_map(
    files: &[super::model::GeneratedWorkflowFileObservation],
) -> std::collections::BTreeMap<&str, &super::model::GeneratedWorkflowFileObservation> {
    files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect()
}

pub(super) fn classify_file(
    observation: Option<&&super::model::GeneratedWorkflowFileObservation>,
    expected_prior: Option<&str>,
    generated: &str,
    has_prior_manifest: bool,
) -> super::model::GeneratedWorkflowFreshness {
    let Some(observation) = observation else {
        return super::model::GeneratedWorkflowFreshness::Missing;
    };
    match observation.kind {
        super::model::GeneratedWorkflowObservedKind::Missing => {
            super::model::GeneratedWorkflowFreshness::Missing
        }
        super::model::GeneratedWorkflowObservedKind::Symlink => {
            super::model::GeneratedWorkflowFreshness::SymlinkConflict
        }
        super::model::GeneratedWorkflowObservedKind::Other => {
            super::model::GeneratedWorkflowFreshness::UnmanagedConflict
        }
        super::model::GeneratedWorkflowObservedKind::Regular => {
            classify_regular_file(observation, expected_prior, generated, has_prior_manifest)
        }
    }
}

pub(super) fn file_blocks_install(
    freshness: &super::model::GeneratedWorkflowFreshness,
    had_expected_prior: bool,
) -> bool {
    matches!(
        freshness,
        super::model::GeneratedWorkflowFreshness::Modified
            | super::model::GeneratedWorkflowFreshness::UnmanagedConflict
            | super::model::GeneratedWorkflowFreshness::UnsupportedVersion
            | super::model::GeneratedWorkflowFreshness::SymlinkConflict
    ) || (*freshness == super::model::GeneratedWorkflowFreshness::Missing && had_expected_prior)
}

pub(super) fn supported_prior_manifest(
    snapshot: &super::model::GeneratedWorkflowDestinationSnapshot,
    adapter_id: &str,
    adapter_version: u32,
) -> Option<super::model::GeneratedWorkflowManifest> {
    match &snapshot.manifest {
        super::model::GeneratedWorkflowManifestObservation::Present(manifest)
            if validate_generated_workflow_manifest(manifest)
                && manifest.adapter_id == adapter_id
                && manifest.adapter_version == adapter_version =>
        {
            Some((**manifest).clone())
        }
        super::model::GeneratedWorkflowManifestObservation::Missing
        | super::model::GeneratedWorkflowManifestObservation::Invalid(_)
        | super::model::GeneratedWorkflowManifestObservation::Symlink
        | super::model::GeneratedWorkflowManifestObservation::Other
        | super::model::GeneratedWorkflowManifestObservation::Present(_) => None,
    }
}

pub(super) fn classify_manifest(
    snapshot: &super::model::GeneratedWorkflowDestinationSnapshot,
    prior: Option<&super::model::GeneratedWorkflowManifest>,
    desired: &super::model::GeneratedWorkflowManifest,
) -> super::model::GeneratedWorkflowFreshness {
    match &snapshot.manifest {
        super::model::GeneratedWorkflowManifestObservation::Missing => {
            super::model::GeneratedWorkflowFreshness::Missing
        }
        super::model::GeneratedWorkflowManifestObservation::Invalid(_) => {
            super::model::GeneratedWorkflowFreshness::UnsupportedVersion
        }
        super::model::GeneratedWorkflowManifestObservation::Symlink => {
            super::model::GeneratedWorkflowFreshness::SymlinkConflict
        }
        super::model::GeneratedWorkflowManifestObservation::Other => {
            super::model::GeneratedWorkflowFreshness::UnmanagedConflict
        }
        super::model::GeneratedWorkflowManifestObservation::Present(_) if prior.is_none() => {
            super::model::GeneratedWorkflowFreshness::UnsupportedVersion
        }
        super::model::GeneratedWorkflowManifestObservation::Present(manifest)
            if manifest.manifest_identity == desired.manifest_identity =>
        {
            super::model::GeneratedWorkflowFreshness::Current
        }
        super::model::GeneratedWorkflowManifestObservation::Present(_) => {
            super::model::GeneratedWorkflowFreshness::StaleSource
        }
    }
}

pub fn generated_workflow_manifest_identity(
    manifest: &super::model::GeneratedWorkflowManifest,
) -> String {
    crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "schema": manifest.schema,
        "generator_version": manifest.generator_version,
        "policy_hash": manifest.policy_hash,
        "catalog_identity": manifest.catalog_identity,
        "adapter_id": manifest.adapter_id,
        "adapter_version": manifest.adapter_version,
        "destination": manifest.destination,
        "files": manifest.files,
        "removal_candidates": manifest.removal_candidates,
        "non_claims": manifest.non_claims,
    }))
}

pub fn validate_generated_workflow_manifest(
    manifest: &super::model::GeneratedWorkflowManifest,
) -> bool {
    let prefix = format!("{}/", manifest.destination);
    let file_paths = manifest
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let workflow_ids = manifest
        .files
        .iter()
        .map(|file| file.workflow_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let removal_paths = manifest
        .removal_candidates
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    manifest.schema == super::model::GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1
        && manifest.generator_version == super::model::GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION
        && !manifest.adapter_id.is_empty()
        && manifest.adapter_version > 0
        && safe_manifest_path(&manifest.destination)
        && valid_blake3(&manifest.policy_hash)
        && valid_blake3(&manifest.catalog_identity)
        && !manifest.files.is_empty()
        && file_paths.len() == manifest.files.len()
        && workflow_ids.len() == manifest.files.len()
        && manifest.files.iter().all(|file| {
            file.relative_path.starts_with(&prefix)
                && safe_manifest_path(&file.relative_path)
                && !file.workflow_id.is_empty()
                && valid_blake3(&file.generated_blake3)
                && valid_blake3(&file.descriptor_identity)
                && file
                    .expected_prior_blake3
                    .as_deref()
                    .is_some_and(valid_blake3)
        })
        && removal_paths.len() == manifest.removal_candidates.len()
        && manifest.removal_candidates.iter().all(|path| {
            path.starts_with(&prefix)
                && safe_manifest_path(path)
                && !file_paths.contains(path.as_str())
        })
        && !manifest.non_claims.is_empty()
        && valid_blake3(&manifest.manifest_identity)
        && manifest.manifest_identity == generated_workflow_manifest_identity(manifest)
}

pub fn inspect_copied_generated_workflow_guide(
    contents: &str,
    workflow_id: &str,
    manifest: Option<&super::model::GeneratedWorkflowManifest>,
    declared_manifest_identity: Option<&str>,
) -> super::model::GeneratedWorkflowCopiedGuideInspection {
    use super::model::GeneratedWorkflowCopiedGuideClassification as Classification;
    let observed_blake3 = crate::verified::evidence::blake3_hex(contents.as_bytes());
    let mut issues = Vec::new();
    let native_layout_current = !super::admission::contains_legacy_layout_reference(contents);
    if !native_layout_current {
        issues.push(super::admission::issue(
            super::model::GENERATED_AGENT_WORKFLOW_ISSUE_GUIDE_LAYOUT,
            "guide",
            "copied guide references a legacy lifecycle layout",
        ));
    }
    let observed_manifest_identity = manifest.map(|manifest| manifest.manifest_identity.clone());
    let (classification, generated_blake3, expected_prior_blake3) = match manifest {
        Some(manifest)
            if validate_generated_workflow_manifest(manifest)
                && declared_manifest_identity == Some(manifest.manifest_identity.as_str()) =>
        {
            match manifest
                .files
                .iter()
                .find(|file| file.workflow_id == workflow_id)
            {
                Some(file) if file.generated_blake3 == observed_blake3 => (
                    Classification::Current,
                    Some(file.generated_blake3.clone()),
                    file.expected_prior_blake3.clone(),
                ),
                Some(file)
                    if file.expected_prior_blake3.as_deref() == Some(observed_blake3.as_str()) =>
                {
                    (
                        Classification::StaleSource,
                        Some(file.generated_blake3.clone()),
                        file.expected_prior_blake3.clone(),
                    )
                }
                Some(file) => (
                    Classification::Modified,
                    Some(file.generated_blake3.clone()),
                    file.expected_prior_blake3.clone(),
                ),
                None => {
                    issues.push(super::admission::issue(
                        super::model::GENERATED_AGENT_WORKFLOW_ISSUE_COPIED_GUIDE,
                        "workflow",
                        format!("manifest does not declare workflow {workflow_id}"),
                    ));
                    (Classification::Unclassified, None, None)
                }
            }
        }
        Some(_) => {
            issues.push(super::admission::issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_COPIED_GUIDE,
                "manifest_identity",
                "declared manifest identity does not match the admitted manifest",
            ));
            (Classification::Unclassified, None, None)
        }
        None => {
            issues.push(super::admission::issue(
                super::model::GENERATED_AGENT_WORKFLOW_ISSUE_COPIED_GUIDE,
                "manifest",
                "copied guide has no admitted manifest identity",
            ));
            (Classification::Unclassified, None, None)
        }
    };
    super::model::GeneratedWorkflowCopiedGuideInspection {
        workflow_id: workflow_id.to_string(),
        classification,
        declared_manifest_identity: declared_manifest_identity.map(str::to_string),
        observed_manifest_identity,
        observed_blake3,
        generated_blake3,
        expected_prior_blake3,
        native_layout_current,
        issues,
    }
}

fn valid_blake3(value: &str) -> bool {
    const BLAKE3_HEX_LENGTH: usize = 64;
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn safe_manifest_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains("//")
        && !path.contains('\\')
        && !path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        && path.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/')
        })
}

fn classify_regular_file(
    observation: &super::model::GeneratedWorkflowFileObservation,
    expected_prior: Option<&str>,
    generated: &str,
    has_prior_manifest: bool,
) -> super::model::GeneratedWorkflowFreshness {
    let observed = observation.content_blake3.as_deref();
    if !has_prior_manifest || expected_prior.is_none() {
        return super::model::GeneratedWorkflowFreshness::UnmanagedConflict;
    }
    if observed == Some(generated) {
        return super::model::GeneratedWorkflowFreshness::Current;
    }
    if observed == expected_prior {
        return super::model::GeneratedWorkflowFreshness::StaleSource;
    }
    super::model::GeneratedWorkflowFreshness::Modified
}
