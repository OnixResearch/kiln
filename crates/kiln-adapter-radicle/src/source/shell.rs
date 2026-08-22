use std::str::FromStr;

use cob_git2 as git2;
use git2::{Oid, Repository};
use kiln_codec::identity_from_bytes;
use kiln_core::source::{
    GitObjectFormat, GitResolutionFacts, SourceNativeFacts, SourceResolution, validate_resolution,
};
use kiln_core::types::{Alias, ExternalRef};
use kiln_provider_api::{
    PortResult, SourceRepositoryBackend, SourceRepositoryObservation, SourceRepositoryPort,
    SourceResolutionObservation,
};
use kiln_radicle_source_contract::{
    CONTRACT_SCHEMA_VERSION, FeatureLevel, SignedRefsObservation, SnapshotEvidence, SourceReceipt,
    Verification, admit_receipt,
};
use radicle_core::RepoId;
use radicle_crypto::PublicKey;

use super::error::{domain, git_failure};
use super::profile::{RadicleSourceProfile, admit_source_profile};
use super::protocol::{parse_oid, reference_target, verify_signed_refs};
use super::snapshot::acquire_snapshot;

const REPOSITORY_FORMAT_VERSION: i64 = 0;

#[derive(Debug)]
pub struct RadicleSourceShell {
    profile: RadicleSourceProfile,
    repository_id: RepoId,
    signer: PublicKey,
    expected_sigrefs: Oid,
    expected_revision: Oid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadicleSourceResult {
    pub resolution: SourceResolution,
    pub receipt: SourceReceipt,
}

impl RadicleSourceShell {
    pub fn new(profile: RadicleSourceProfile) -> PortResult<Self> {
        let _admitted = admit_source_profile(&profile)?;
        let repository_id = RepoId::from_str(&profile.request.repository_id).map_err(|_| {
            domain(
                "radicle_source_rid",
                "radicle_source.request.repository_id",
                "Radicle source request contains an invalid RID",
            )
        })?;
        let signer = PublicKey::from_str(&profile.request.signer_node_id).map_err(|_| {
            domain(
                "radicle_source_signer",
                "radicle_source.request.signer_node_id",
                "Radicle source request contains an invalid signer identity",
            )
        })?;
        let expected_sigrefs = parse_oid(
            &profile.request.expected_sigrefs_commit,
            "radicle_source.request.expected_sigrefs_commit",
        )?;
        let expected_revision = parse_oid(
            &profile.request.expected_revision,
            "radicle_source.request.expected_revision",
        )?;
        Ok(Self {
            profile,
            repository_id,
            signer,
            expected_sigrefs,
            expected_revision,
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &RadicleSourceProfile {
        &self.profile
    }

    fn open(&self) -> PortResult<Repository> {
        Repository::open(&self.profile.repository_path).map_err(git_failure)
    }

    fn inspect_repository(&self, repository: &Repository) -> PortResult<()> {
        let config = repository.config().map_err(git_failure)?;
        let format_version = config
            .get_i64("core.repositoryformatversion")
            .unwrap_or(REPOSITORY_FORMAT_VERSION);
        let non_sha1 = config
            .get_string("extensions.objectformat")
            .is_ok_and(|value| !value.eq_ignore_ascii_case("sha1"));
        let partial = config.get_string("extensions.partialclone").is_ok();
        let worktree_config = config
            .get_bool("extensions.worktreeconfig")
            .unwrap_or(false);
        let replacement = repository
            .references_glob("refs/replace/*")
            .map_err(git_failure)?
            .next()
            .is_some();
        let alternate = repository.path().join("objects/info/alternates").exists();
        if !repository.is_bare()
            || format_version != REPOSITORY_FORMAT_VERSION
            || non_sha1
            || partial
            || worktree_config
            || replacement
            || alternate
        {
            return Err(domain(
                "radicle_source_repository_feature",
                "radicle_source.repository",
                "Radicle repository uses a feature outside the read-only source cohort",
            ));
        }
        Ok(())
    }

    fn verify(&self, repository: &Repository) -> PortResult<super::protocol::VerifiedSignedRefs> {
        verify_signed_refs(
            &self.profile,
            repository,
            self.repository_id,
            &self.signer,
            self.expected_sigrefs,
            self.expected_revision,
        )
    }

    fn execute(&self) -> PortResult<(RadicleSourceResult, Vec<u8>)> {
        let admitted = admit_source_profile(&self.profile)?;
        let repository = self.open()?;
        self.inspect_repository(&repository)?;
        let verified = self.verify(&repository)?;
        let snapshot = acquire_snapshot(&self.profile, &repository, verified.signed_revision)?;
        let observed_after = reference_target(&repository, &verified.namespaced_reference)?;
        if observed_after != verified.sigrefs_commit {
            return Err(domain(
                "radicle_source_reference_moved",
                "radicle_source.sigrefs",
                "Radicle signed-reference commit moved during source acquisition",
            ));
        }
        let receipt = SourceReceipt {
            schema_version: CONTRACT_SCHEMA_VERSION,
            request: self.profile.request.clone(),
            observation: SignedRefsObservation {
                schema_version: CONTRACT_SCHEMA_VERSION,
                profile_id: self.profile.request.profile_id.clone(),
                repository_id: self.profile.request.repository_id.clone(),
                signer_node_id: self.profile.request.signer_node_id.clone(),
                signed_reference: self.profile.request.signed_reference.clone(),
                namespaced_sigrefs_reference: verified.namespaced_reference,
                sigrefs_commit_before: verified.sigrefs_commit.to_string(),
                sigrefs_commit_after: observed_after.to_string(),
                sigrefs_parent: verified.sigrefs_parent.map(|oid| oid.to_string()),
                identity_root_commit: verified.identity_root_commit.to_string(),
                signed_revision: verified.signed_revision.to_string(),
                namespaced_revision: verified.signed_revision.to_string(),
                feature_level: FeatureLevel::Parent,
                signature: Verification::Verified,
                repository_identity: Verification::Verified,
                namespace_target: Verification::Verified,
            },
            snapshot: SnapshotEvidence {
                schema_version: CONTRACT_SCHEMA_VERSION,
                revision: verified.signed_revision.to_string(),
                native_tree: snapshot.native_tree.to_string(),
                canonical_tree_blake3: snapshot.canonical_tree_blake3.to_hex().to_string(),
                entry_count: snapshot.entry_count,
                total_bytes: snapshot.total_bytes,
            },
        };
        admit_receipt(&receipt).map_err(|_| {
            domain(
                "radicle_source_receipt",
                "radicle_source.receipt",
                "Radicle source receipt failed contract admission",
            )
        })?;

        let repository_identity =
            ExternalRef::new("radicle", "repository", &self.profile.request.repository_id)?;
        let symbolic_reference = ExternalRef::new(
            "radicle",
            "reference",
            &self.profile.request.signed_reference,
        )?;
        let exact_revision =
            ExternalRef::new("radicle", "revision", &verified.signed_revision.to_string())?;
        let native_tree =
            ExternalRef::new("radicle", "git-tree", &snapshot.native_tree.to_string())?;
        let resolution = SourceResolution {
            source_provider: admitted.id,
            repository: repository_identity.clone(),
            symbolic_reference: symbolic_reference.clone(),
            exact_revision: exact_revision.clone(),
            canonical_tree: identity_from_bytes(&snapshot.manifest).map_err(|_| {
                domain(
                    "radicle_source_tree_identity",
                    "radicle_source.snapshot",
                    "Radicle canonical tree identity could not be computed",
                )
            })?,
            native_facts: SourceNativeFacts::Git(GitResolutionFacts {
                repository: repository_identity,
                symbolic_reference,
                exact_commit: exact_revision.clone(),
                native_tree,
                observed_ref_before: exact_revision.clone(),
                observed_ref_after: exact_revision,
                object_format: GitObjectFormat::Sha1,
                partial_repository: false,
                replacement_objects: false,
                alternate_object_database: false,
                signature_observed: true,
            }),
        };
        validate_resolution(&resolution)?;
        Ok((
            RadicleSourceResult {
                resolution,
                receipt,
            },
            snapshot.manifest,
        ))
    }

    // r[impl kiln.radicle.shell_source]
    pub fn resolve_and_acquire(&self) -> PortResult<RadicleSourceResult> {
        self.execute().map(|(result, _manifest)| result)
    }
}

impl SourceRepositoryPort for RadicleSourceShell {
    fn observe(&self) -> PortResult<SourceRepositoryObservation> {
        let repository = self.open()?;
        self.inspect_repository(&repository)?;
        Ok(SourceRepositoryObservation {
            repository: ExternalRef::new(
                "radicle",
                "repository",
                &self.profile.request.repository_id,
            )?,
            backend: SourceRepositoryBackend::Git,
            operation: Some(ExternalRef::new(
                "radicle",
                "sigrefs",
                &self.profile.request.expected_sigrefs_commit,
            )?),
        })
    }

    fn resolve(&self, symbolic: &ExternalRef) -> PortResult<SourceResolutionObservation> {
        if symbolic.provider_namespace.as_str() != "radicle"
            || symbolic.native_kind.as_str() != "reference"
            || symbolic.native_id.as_str() != self.profile.request.signed_reference
        {
            return Err(domain(
                "radicle_source_reference_substitution",
                "radicle_source.reference",
                "Radicle source resolution request differs from the configured reference",
            ));
        }
        let repository = self.open()?;
        self.inspect_repository(&repository)?;
        let verified = self.verify(&repository)?;
        let alias = self
            .profile
            .request
            .signed_reference
            .strip_prefix("refs/heads/")
            .map(Alias::new)
            .transpose()?;
        Ok(SourceResolutionObservation {
            symbolic: symbolic.clone(),
            native_change: Some(ExternalRef::new(
                "radicle",
                "sigrefs",
                &verified.sigrefs_commit.to_string(),
            )?),
            exact_revisions: vec![ExternalRef::new(
                "radicle",
                "revision",
                &verified.signed_revision.to_string(),
            )?],
            alias,
            conflicted_alias: false,
            divergent_change: false,
        })
    }

    fn acquire_canonical_manifest(&self, commit: &ExternalRef) -> PortResult<Vec<u8>> {
        if commit.provider_namespace.as_str() != "radicle"
            || commit.native_kind.as_str() != "revision"
            || commit.native_id.as_str() != self.profile.request.expected_revision
        {
            return Err(domain(
                "radicle_source_revision_substitution",
                "radicle_source.revision",
                "Radicle source acquisition request differs from the exact revision",
            ));
        }
        self.execute().map(|(_result, manifest)| manifest)
    }
}
