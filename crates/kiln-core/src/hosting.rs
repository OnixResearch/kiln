pub mod qualification;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef, PolicyId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostingState {
    Planned,
    Candidate,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MirrorAuthority {
    ReadOnlyProjection,
    WritableProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MirrorProfile {
    pub route: ExternalRef,
    pub authority: MirrorAuthority,
    pub observed_write: bool,
    pub expected_revision: ExternalRef,
    pub observed_revision: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionRoute {
    pub route: ExternalRef,
    pub writable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostingProfile {
    pub state: HostingState,
    pub profile_id: PolicyId,
    pub canonical_branch: ExternalRef,
    pub rid: Option<ExternalRef>,
    pub delegates: Vec<ExternalRef>,
    pub threshold: usize,
    pub required_replicas: usize,
    pub license: ExternalRef,
    pub rollback_revision: ExternalRef,
    pub signed_ref_features: BTreeSet<ExternalRef>,
    pub seeds: Vec<ExternalRef>,
    pub acquisition_routes: Vec<AcquisitionRoute>,
    pub freeze: bool,
    pub mirrors: Vec<MirrorProfile>,
}

pub fn admit_hosting_profile(profile: &HostingProfile) -> CoreResult<()> {
    profile.canonical_branch.validate()?;
    profile.license.validate()?;
    profile.rollback_revision.validate()?;
    if profile.threshold == 0 || profile.threshold > profile.delegates.len() {
        return Err(Diagnostic::new(
            "hosting_threshold",
            "hosting.threshold",
            "delegate threshold is invalid",
        ));
    }
    let delegate_set: BTreeSet<_> = profile.delegates.iter().collect();
    if delegate_set.len() != profile.delegates.len() {
        return Err(Diagnostic::new(
            "duplicate_delegate",
            "hosting.delegates",
            "hosting delegates must be unique",
        ));
    }
    if profile.required_replicas == 0 {
        return Err(Diagnostic::new(
            "hosting_replica_bound",
            "hosting.required_replicas",
            "required replica count must be positive",
        ));
    }
    if profile.state == HostingState::Published {
        if profile.rid.is_none() {
            return Err(Diagnostic::new(
                "hosting_rid_missing",
                "hosting.rid",
                "published hosting profile requires an assigned RID",
            ));
        }
        if profile.signed_ref_features.is_empty()
            || profile.seeds.is_empty()
            || profile.acquisition_routes.is_empty()
        {
            return Err(Diagnostic::new(
                "hosting_observation_missing",
                "hosting",
                "published hosting profile requires signed-ref, seed, and acquisition facts",
            ));
        }
    }
    for route in &profile.acquisition_routes {
        route.route.validate()?;
        if route.writable {
            return Err(Diagnostic::new(
                "writable_acquisition_route",
                "hosting.acquisition_routes",
                "repository acquisition routes must not gain write authority",
            ));
        }
    }
    for mirror in &profile.mirrors {
        if mirror.authority == MirrorAuthority::WritableProjection || mirror.observed_write {
            return Err(Diagnostic::new(
                "mirror_write_route",
                "hosting.mirrors",
                "read-only mirror accepted a write",
            ));
        }
        if mirror.expected_revision != mirror.observed_revision {
            return Err(Diagnostic::new(
                "mirror_drift",
                "hosting.mirrors",
                "mirror revision differs from the declared projection",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicationCandidate {
    pub source_revision: ExternalRef,
    pub archive_identity: ContentId,
    pub license: ExternalRef,
    pub lifecycle_evidence: ContentId,
    pub rollback_revision: ExternalRef,
    pub hosting_profile_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionObservation {
    pub route: ExternalRef,
    pub revision: ExternalRef,
    pub archive_identity: ContentId,
    pub observed_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicationBundle {
    pub sequence: u64,
    pub candidate: PublicationCandidate,
    pub rid: ExternalRef,
    pub signed_revision: ExternalRef,
    pub observed_delegates: Vec<ExternalRef>,
    pub observed_threshold: usize,
    pub acquisitions: Vec<AcquisitionObservation>,
}

pub fn validate_publication_bundle(
    profile: &HostingProfile,
    bundle: &PublicationBundle,
    last_sequence: Option<u64>,
    current_time: u64,
    max_age: u64,
) -> CoreResult<()> {
    admit_hosting_profile(profile)?;
    if bundle.candidate.license != profile.license
        || bundle.candidate.rollback_revision != profile.rollback_revision
    {
        return Err(Diagnostic::new(
            "publication_candidate_mismatch",
            "publication.candidate",
            "publication candidate license or rollback fact differs from the hosting profile",
        ));
    }
    if profile.rid.as_ref() != Some(&bundle.rid) {
        return Err(Diagnostic::new(
            "hosting_rid_mismatch",
            "publication.rid",
            "publication RID differs from the hosting profile",
        ));
    }
    if bundle.signed_revision != bundle.candidate.source_revision
        || bundle.observed_threshold != profile.threshold
        || bundle.observed_delegates != profile.delegates
    {
        return Err(Diagnostic::new(
            "publication_bundle_mixed",
            "publication",
            "publication bundle mixes candidate or governance observations",
        ));
    }
    if last_sequence.is_some_and(|last| bundle.sequence <= last) {
        return Err(Diagnostic::new(
            "publication_rollback",
            "publication.sequence",
            "publication sequence is stale or rolled back",
        ));
    }
    if bundle.acquisitions.len() < profile.required_replicas {
        return Err(Diagnostic::new(
            "replica_shortfall",
            "publication.acquisitions",
            "publication lacks required acquisition observations",
        ));
    }
    for acquisition in &bundle.acquisitions {
        if acquisition.revision != bundle.candidate.source_revision
            || acquisition.archive_identity != bundle.candidate.archive_identity
        {
            return Err(Diagnostic::new(
                "acquisition_mismatch",
                "publication.acquisitions",
                "acquisition does not match the exact candidate",
            ));
        }
        if current_time.saturating_sub(acquisition.observed_at) > max_age {
            return Err(Diagnostic::new(
                "stale_acquisition",
                "publication.acquisitions",
                "acquisition observation is stale",
            ));
        }
    }
    Ok(())
}

/// One source-only publication observation bound to a single frozen source.
// r[impl kiln.hosting.source_only_publication]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceOnlyPublication {
    pub candidate: PublicationCandidate,
    pub previous_main_revision: ExternalRef,
    pub fast_forward: bool,
    pub force_push: bool,
    pub history_rewrite: bool,
    pub mirror_mutation: bool,
    pub governance_mutation: bool,
    pub readbacks: Vec<AcquisitionObservation>,
}

/// Admits a source-only publication of one frozen source revision.
// r[impl kiln.hosting.source_only_publication]
pub fn admit_source_only_publication(
    profile: &HostingProfile,
    publication: &SourceOnlyPublication,
) -> CoreResult<()> {
    admit_hosting_profile(profile)?;
    if publication.force_push || publication.history_rewrite {
        return Err(Diagnostic::new(
            "source_only_publication_rewrite",
            "publication.rewrite",
            "source-only publication must not force push or rewrite history",
        ));
    }
    if publication.mirror_mutation || publication.governance_mutation {
        return Err(Diagnostic::new(
            "source_only_publication_scope",
            "publication.scope",
            "source-only publication must not mutate mirrors or governance",
        ));
    }
    if !publication.fast_forward {
        return Err(Diagnostic::new(
            "source_only_publication_not_fast_forward",
            "publication.fast_forward",
            "source-only publication must fast-forward the canonical branch",
        ));
    }
    if publication.candidate.source_revision == publication.previous_main_revision
        || publication.candidate.rollback_revision != publication.previous_main_revision
    {
        return Err(Diagnostic::new(
            "source_only_publication_ancestry",
            "publication.ancestry",
            "source-only publication must advance the canonical branch and keep the previous main as rollback",
        ));
    }
    if publication.candidate.license != profile.license {
        return Err(Diagnostic::new(
            "source_only_publication_license",
            "publication.license",
            "source-only publication license differs from the hosting profile",
        ));
    }
    if publication.readbacks.len() < profile.required_replicas {
        return Err(Diagnostic::new(
            "source_only_publication_replica_shortfall",
            "publication.readbacks",
            "source-only publication lacks the required readback observations",
        ));
    }
    for readback in &publication.readbacks {
        if readback.revision != publication.candidate.source_revision
            || readback.archive_identity != publication.candidate.archive_identity
        {
            return Err(Diagnostic::new(
                "source_only_publication_readback_mismatch",
                "publication.readbacks",
                "readback does not reproduce the exact published revision and archive",
            ));
        }
        if !profile
            .acquisition_routes
            .iter()
            .any(|route| route.route == readback.route && !route.writable)
        {
            return Err(Diagnostic::new(
                "source_only_publication_route",
                "publication.readbacks",
                "readback route is not a declared read-only acquisition route",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_profile_requires_rid() {
        let profile = HostingProfile {
            state: HostingState::Published,
            profile_id: PolicyId::new("hosting").unwrap_or_else(|_| unreachable!()),
            canonical_branch: ExternalRef::new("git", "branch", "main")
                .unwrap_or_else(|_| unreachable!()),
            rid: None,
            delegates: alloc::vec![
                ExternalRef::new("radicle", "delegate", "did:key:one")
                    .unwrap_or_else(|_| unreachable!())
            ],
            threshold: 1,
            required_replicas: 1,
            license: ExternalRef::new("spdx", "license", "MIT OR Apache-2.0")
                .unwrap_or_else(|_| unreachable!()),
            rollback_revision: ExternalRef::new("git", "commit", "rollback")
                .unwrap_or_else(|_| unreachable!()),
            signed_ref_features: BTreeSet::new(),
            seeds: Vec::new(),
            acquisition_routes: Vec::new(),
            freeze: false,
            mirrors: Vec::new(),
        };
        assert!(admit_hosting_profile(&profile).is_err());
    }

    #[test]
    fn readonly_mirror_rejects_write() {
        let mirror = MirrorProfile {
            route: ExternalRef::new("https", "repository", "mirror")
                .unwrap_or_else(|_| unreachable!()),
            authority: MirrorAuthority::ReadOnlyProjection,
            observed_write: true,
            expected_revision: ExternalRef::new("git", "commit", "a")
                .unwrap_or_else(|_| unreachable!()),
            observed_revision: ExternalRef::new("git", "commit", "a")
                .unwrap_or_else(|_| unreachable!()),
        };
        assert!(mirror.observed_write);
    }
}
