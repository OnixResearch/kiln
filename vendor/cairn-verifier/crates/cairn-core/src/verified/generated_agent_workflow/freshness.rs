use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::verified::evidence::canonical_json_hash;

use super::model::{
    GeneratedWorkflowDestinationSnapshot, GeneratedWorkflowFileObservation,
    GeneratedWorkflowFreshness, GeneratedWorkflowManifest, GeneratedWorkflowManifestObservation,
    GeneratedWorkflowObservedKind, GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION,
    GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1,
};

pub(super) fn observation_map(
    files: &[GeneratedWorkflowFileObservation],
) -> BTreeMap<&str, &GeneratedWorkflowFileObservation> {
    files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect()
}

pub(super) fn classify_file(
    observation: Option<&&GeneratedWorkflowFileObservation>,
    expected_prior: Option<&str>,
    generated: &str,
    has_prior_manifest: bool,
) -> GeneratedWorkflowFreshness {
    let Some(observation) = observation else {
        return GeneratedWorkflowFreshness::Missing;
    };
    match observation.kind {
        GeneratedWorkflowObservedKind::Missing => GeneratedWorkflowFreshness::Missing,
        GeneratedWorkflowObservedKind::Symlink => GeneratedWorkflowFreshness::SymlinkConflict,
        GeneratedWorkflowObservedKind::Other => GeneratedWorkflowFreshness::UnmanagedConflict,
        GeneratedWorkflowObservedKind::Regular => {
            classify_regular_file(observation, expected_prior, generated, has_prior_manifest)
        }
    }
}

pub(super) fn file_blocks_install(
    freshness: &GeneratedWorkflowFreshness,
    had_expected_prior: bool,
) -> bool {
    matches!(
        freshness,
        GeneratedWorkflowFreshness::Modified
            | GeneratedWorkflowFreshness::UnmanagedConflict
            | GeneratedWorkflowFreshness::UnsupportedVersion
            | GeneratedWorkflowFreshness::SymlinkConflict
    ) || (*freshness == GeneratedWorkflowFreshness::Missing && had_expected_prior)
}

pub(super) fn supported_prior_manifest(
    snapshot: &GeneratedWorkflowDestinationSnapshot,
    adapter_id: &str,
    adapter_version: u32,
) -> Option<GeneratedWorkflowManifest> {
    match &snapshot.manifest {
        GeneratedWorkflowManifestObservation::Present(manifest)
            if validate_generated_workflow_manifest(manifest)
                && manifest.adapter_id == adapter_id
                && manifest.adapter_version == adapter_version =>
        {
            Some((**manifest).clone())
        }
        GeneratedWorkflowManifestObservation::Missing
        | GeneratedWorkflowManifestObservation::Invalid(_)
        | GeneratedWorkflowManifestObservation::Symlink
        | GeneratedWorkflowManifestObservation::Other
        | GeneratedWorkflowManifestObservation::Present(_) => None,
    }
}

pub(super) fn classify_manifest(
    snapshot: &GeneratedWorkflowDestinationSnapshot,
    prior: Option<&GeneratedWorkflowManifest>,
    desired: &GeneratedWorkflowManifest,
) -> GeneratedWorkflowFreshness {
    match &snapshot.manifest {
        GeneratedWorkflowManifestObservation::Missing => GeneratedWorkflowFreshness::Missing,
        GeneratedWorkflowManifestObservation::Invalid(_) => {
            GeneratedWorkflowFreshness::UnsupportedVersion
        }
        GeneratedWorkflowManifestObservation::Symlink => {
            GeneratedWorkflowFreshness::SymlinkConflict
        }
        GeneratedWorkflowManifestObservation::Other => {
            GeneratedWorkflowFreshness::UnmanagedConflict
        }
        GeneratedWorkflowManifestObservation::Present(_) if prior.is_none() => {
            GeneratedWorkflowFreshness::UnsupportedVersion
        }
        GeneratedWorkflowManifestObservation::Present(manifest)
            if manifest.manifest_identity == desired.manifest_identity =>
        {
            GeneratedWorkflowFreshness::Current
        }
        GeneratedWorkflowManifestObservation::Present(_) => GeneratedWorkflowFreshness::StaleSource,
    }
}

pub fn generated_workflow_manifest_identity(manifest: &GeneratedWorkflowManifest) -> String {
    canonical_json_hash(&json!({
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

pub fn validate_generated_workflow_manifest(manifest: &GeneratedWorkflowManifest) -> bool {
    let prefix = format!("{}/", manifest.destination);
    let file_paths = manifest
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    let workflow_ids = manifest
        .files
        .iter()
        .map(|file| file.workflow_id.as_str())
        .collect::<BTreeSet<_>>();
    let removal_paths = manifest
        .removal_candidates
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    manifest.schema == GENERATED_AGENT_WORKFLOW_MANIFEST_SCHEMA_V1
        && manifest.generator_version == GENERATED_AGENT_WORKFLOW_GENERATOR_VERSION
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
    observation: &GeneratedWorkflowFileObservation,
    expected_prior: Option<&str>,
    generated: &str,
    has_prior_manifest: bool,
) -> GeneratedWorkflowFreshness {
    let observed = observation.content_blake3.as_deref();
    if !has_prior_manifest || expected_prior.is_none() {
        return GeneratedWorkflowFreshness::UnmanagedConflict;
    }
    if observed == Some(generated) {
        return GeneratedWorkflowFreshness::Current;
    }
    if observed == expected_prior {
        return GeneratedWorkflowFreshness::StaleSource;
    }
    GeneratedWorkflowFreshness::Modified
}
