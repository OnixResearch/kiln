use super::api::{error, json_response, query};
use super::{ApiResponse, Engine, Role};
use crate::profile::ProjectProfile;
use crate::snapshot::{SnapshotManifest, SnapshotProvenance, SnapshotRecord};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::source::SourceResolution;
use kiln_core::types::ProviderProfileId;

pub(super) fn validate(state: &super::model::DurableState, maximum: usize) -> Result<(), String> {
    if state.snapshots.len() > maximum {
        return Err("stored snapshot retention exceeded".into());
    }
    for (id, record) in &state.snapshots {
        if record.manifest.identity()? != *id
            || record.provenance.manifest_id != *id
            || record.provenance.base_revision != record.manifest.base_revision
            || !crate::snapshot::hex(&record.provenance.synthetic_revision, 20)
        {
            return Err("stored snapshot identity conflict".into());
        }
    }
    for run in state.runs.values() {
        if let Some(provenance) = &run.dirty_snapshot {
            let record = state
                .snapshots
                .get(&provenance.manifest_id)
                .ok_or("stored dirty input missing")?;
            if provenance.base_revision != record.provenance.base_revision
                || provenance.synthetic_revision != record.provenance.synthetic_revision
                || run.project_id != record.manifest.project_id
                || run.source.exact_revision.native_id.as_str() != provenance.synthetic_revision
            {
                return Err("stored dirty input linkage conflict".into());
            }
        }
    }
    Ok(())
}

pub(super) fn resolve_snapshot(
    project: &ProjectProfile,
    record: &SnapshotRecord,
) -> Result<SourceResolution, String> {
    let base = super::queue::resolve_source(
        project,
        &record.manifest.reference,
        Some(&record.manifest.base_revision),
    )?;
    if record.manifest.project_id != project.id
        || record.manifest.identity()? != record.provenance.manifest_id
        || record.manifest.base_revision != record.provenance.base_revision
    {
        return Err("snapshot_identity_mismatch".into());
    }
    let synthetic = crate::snapshot::reconstruct(
        &project.repository_path,
        &record.manifest,
        &project.source_limits,
    )?;
    if synthetic.revision != record.provenance.synthetic_revision {
        return Err("snapshot_revision_mismatch".into());
    }
    let provider = ProviderProfileId::new(format!("web-git-{}", project.id))
        .map_err(|_| "source_profile_invalid")?;
    let adapter = LocalGitAdapter::new(
        synthetic.repository_path.clone(),
        synthetic.revision.clone(),
        GitSourceProfile::pinned_local(provider.clone(), project.source_limits.clone()),
    )
    .map_err(|_| "snapshot_source_invalid")?;
    let mut resolution =
        resolve_and_acquire(&adapter, provider).map_err(|_| "snapshot_source_unavailable")?;
    // The reviewed base grants repository authority; the independent object store
    // is an acquisition location, never a replacement repository identity.
    resolution.repository = base.repository.clone();
    if let kiln_core::source::SourceNativeFacts::Git(facts) = &mut resolution.native_facts {
        facts.repository = base.repository;
    } else {
        return Err("snapshot_source_invalid".into());
    }
    kiln_core::source::validate_resolution(&resolution).map_err(|_| "snapshot_source_invalid")?;
    Ok(resolution)
}
impl Engine {
    pub(super) fn snapshot_route(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<Option<ApiResponse>, String> {
        let (path, params) = match query(target) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        if path != "/api/snapshots" && !path.starts_with("/api/snapshots/") {
            return Ok(None);
        }
        let response = if !params.is_empty() || body.len() > self.profile.bounds.max_request_bytes {
            error(400, "snapshot_request_invalid")
        } else if method == "POST" && path == "/api/snapshots" {
            if role != Role::Operator {
                error(403, "snapshot_operator_required")
            } else {
                self.upload_snapshot(body)
            }
        } else if method == "GET" && body.is_empty() {
            if role == Role::Viewer {
                error(403, "snapshot_worker_required")
            } else {
                let suffix = path.strip_prefix("/api/snapshots/").unwrap_or("");
                let (project, revision) = suffix.split_once('/').unwrap_or(("", ""));
                if !crate::profile::identifier(project) || !crate::snapshot::hex(revision, 20) {
                    error(400, "snapshot_request_invalid")
                } else if let Some(record) = self.state.snapshots.values().find(|s| {
                    s.manifest.project_id == project && s.provenance.synthetic_revision == revision
                }) {
                    // Download authority is retained only for an exact admitted candidate.
                    if !self.state.runs.values().any(|r| {
                        r.project_id == project
                            && r.source.exact_revision.native_id.as_str() == revision
                    }) {
                        error(404, "snapshot_candidate_missing")
                    } else {
                        json_response(200, record)
                    }
                } else {
                    error(404, "snapshot_missing")
                }
            }
        } else {
            error(404, "snapshot_route_missing")
        };
        response.map(Some)
    }
    fn upload_snapshot(&mut self, body: &[u8]) -> Result<ApiResponse, String> {
        let manifest: SnapshotManifest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "snapshot_manifest_invalid"),
        };
        let manifest_id = match manifest.identity() {
            Ok(v) => v,
            Err(code) => return error(400, &code),
        };
        if let Some(record) = self.state.snapshots.get(&manifest_id) {
            return json_response(200, record);
        }
        if self.state.snapshots.len() >= self.profile.bounds.max_runs {
            return error(409, "snapshot_retention");
        }
        let Some(project) = self
            .profile
            .projects
            .iter()
            .find(|p| p.id == manifest.project_id)
            .cloned()
        else {
            return error(404, "project_not_allowlisted");
        };
        if !self.state.enrolled.contains(&project.id) {
            return error(409, "project_not_enrolled");
        }
        if let Err(code) = super::queue::resolve_source(
            &project,
            &manifest.reference,
            Some(&manifest.base_revision),
        ) {
            return error(400, &code);
        }
        let synthetic = match crate::snapshot::reconstruct(
            &project.repository_path,
            &manifest,
            &project.source_limits,
        ) {
            Ok(v) => v,
            Err(code) => return error(400, &code),
        };
        let record = SnapshotRecord {
            provenance: SnapshotProvenance {
                manifest_id: manifest_id.clone(),
                base_revision: manifest.base_revision.clone(),
                synthetic_revision: synthetic.revision.clone(),
            },
            manifest,
        };
        self.state
            .snapshots
            .insert(manifest_id.clone(), record.clone());
        self.persist()?;
        json_response(201, record)
    }
}
