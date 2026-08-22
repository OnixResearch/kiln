use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use blake3::Hash;
use cob_git2 as git2;
use git2::{ObjectType, Oid, Repository, Tree};
use kiln_codec::identity_from_bytes;
use kiln_core::source::{
    GitEntryKind, GitObjectFormat, GitResolutionFacts, GitSnapshotEntry, GitSnapshotLimits,
    SourceNativeFacts, SourceResolution, build_git_snapshot_manifest, validate_resolution,
};
use kiln_core::types::{Alias, Diagnostic, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, SourceRepositoryBackend,
    SourceRepositoryObservation, SourceRepositoryPort, SourceResolutionObservation,
};
use kiln_radicle_source_contract::{
    CONTRACT_SCHEMA_VERSION, FeatureLevel, SignedRefsObservation, SnapshotEvidence, SourceReceipt,
    Verification, admit_receipt,
};
use radicle_core::RepoId;
use radicle_crypto::signature::Verifier as _;
use radicle_crypto::{PublicKey, Signature as RadicleSignature, VerifyingKey};
use radicle_git_ref_format::RefString;

use super::profile::{RadicleSourceProfile, admit_source_profile};

const REFS_BLOB_PATH: &str = "refs";
const SIGNATURE_BLOB_PATH: &str = "signature";
const IDENTITY_DOCUMENT_PATH: &str = "embeds/radicle.json";
const IDENTITY_ROOT_REFERENCE: &str = "refs/rad/root";
const SIGREFS_PARENT_REFERENCE: &str = "refs/rad/sigrefs-parent";
const SIGREFS_REFERENCE: &str = "refs/rad/sigrefs";
const REPOSITORY_FORMAT_VERSION: i64 = 0;
const MODE_REGULAR: i32 = 0o100_644;
const MODE_EXECUTABLE: i32 = 0o100_755;
const MODE_SYMLINK: i32 = 0o120_000;
const MODE_GITLINK: i32 = 0o160_000;

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

#[derive(Debug)]
struct VerifiedSignedRefs {
    sigrefs_commit: Oid,
    sigrefs_parent: Option<Oid>,
    identity_root_commit: Oid,
    signed_revision: Oid,
    namespaced_reference: String,
}

#[derive(Debug)]
struct AcquiredSnapshot {
    manifest: Vec<u8>,
    native_tree: Oid,
    entry_count: u64,
    total_bytes: u64,
    canonical_tree_blake3: Hash,
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

    fn sigrefs_reference(&self) -> String {
        format!(
            "refs/namespaces/{}/{SIGREFS_REFERENCE}",
            self.profile.request.signer_node_id
        )
    }

    fn namespaced_source_reference(&self) -> String {
        format!(
            "refs/namespaces/{}/{}",
            self.profile.request.signer_node_id, self.profile.request.signed_reference
        )
    }

    fn reference_target(repository: &Repository, reference: &str) -> PortResult<Oid> {
        repository
            .find_reference(reference)
            .map_err(git_failure)?
            .target()
            .ok_or_else(|| {
                domain(
                    "radicle_source_symbolic_reference",
                    "radicle_source.reference",
                    "Radicle source reference is not a direct object reference",
                )
            })
    }

    fn read_blob(
        repository: &Repository,
        commit: &git2::Commit<'_>,
        path: &str,
    ) -> PortResult<Vec<u8>> {
        let tree = commit.tree().map_err(git_failure)?;
        let entry = tree.get_path(Path::new(path)).map_err(git_failure)?;
        if entry.kind() != Some(ObjectType::Blob) {
            return Err(domain(
                "radicle_source_blob_kind",
                "radicle_source.sigrefs",
                "Radicle signed-reference entry is not a blob",
            ));
        }
        let blob = repository.find_blob(entry.id()).map_err(git_failure)?;
        Ok(blob.content().to_vec())
    }

    fn parse_refs(&self, bytes: &[u8]) -> PortResult<(BTreeMap<String, Oid>, Vec<u8>)> {
        let byte_count = u64::try_from(bytes.len()).map_err(|_| {
            domain(
                "radicle_source_sigrefs_bound",
                "radicle_source.sigrefs",
                "Radicle signed-reference bytes exceed the supported range",
            )
        })?;
        if byte_count > self.profile.request.limits.max_total_bytes {
            return Err(domain(
                "radicle_source_sigrefs_bound",
                "radicle_source.sigrefs",
                "Radicle signed-reference bytes exceed the request bound",
            ));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| {
            domain(
                "radicle_source_sigrefs_encoding",
                "radicle_source.sigrefs",
                "Radicle signed-reference bytes are not valid UTF-8",
            )
        })?;
        let mut refs = BTreeMap::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            if refs.len()
                >= usize::try_from(self.profile.request.limits.max_entries).map_err(|_| {
                    domain(
                        "radicle_source_sigrefs_bound",
                        "radicle_source.sigrefs",
                        "Radicle signed-reference entry bound exceeds this platform",
                    )
                })?
            {
                return Err(domain(
                    "radicle_source_sigrefs_bound",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference entry count exceeds the request bound",
                ));
            }
            let (oid, name) = line.split_once(' ').ok_or_else(|| {
                domain(
                    "radicle_source_sigrefs_format",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference line has an invalid shape",
                )
            })?;
            let _validated = RefString::try_from(name.to_string()).map_err(|_| {
                domain(
                    "radicle_source_sigrefs_name",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference name is invalid",
                )
            })?;
            let oid = parse_oid(oid, "radicle_source.sigrefs")?;
            if oid.is_zero() || refs.insert(name.to_string(), oid).is_some() {
                return Err(domain(
                    "radicle_source_sigrefs_entry",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference entry is zero or duplicated",
                ));
            }
        }
        if refs.is_empty() {
            return Err(domain(
                "radicle_source_sigrefs_empty",
                "radicle_source.sigrefs",
                "Radicle signed-reference map is empty",
            ));
        }
        let mut canonical = Vec::new();
        for (name, oid) in &refs {
            canonical.extend_from_slice(oid.to_string().as_bytes());
            canonical.push(b' ');
            canonical.extend_from_slice(name.as_bytes());
            canonical.push(b'\n');
        }
        Ok((refs, canonical))
    }

    // r[impl kiln.radicle.shell_source]
    fn verify_signed_refs(&self, repository: &Repository) -> PortResult<VerifiedSignedRefs> {
        let namespaced_reference = self.sigrefs_reference();
        let sigrefs_commit = Self::reference_target(repository, &namespaced_reference)?;
        if sigrefs_commit != self.expected_sigrefs {
            return Err(domain(
                "radicle_source_sigrefs_substitution",
                "radicle_source.sigrefs",
                "Radicle signed-reference commit differs from the exact request",
            ));
        }
        let commit = repository
            .find_commit(sigrefs_commit)
            .map_err(git_failure)?;
        let parent = match commit.parent_count() {
            0 => None,
            1 => Some(commit.parent_id(0).map_err(git_failure)?),
            _ => {
                return Err(domain(
                    "radicle_source_sigrefs_history",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference history is not linear",
                ));
            }
        };
        let refs_bytes = Self::read_blob(repository, &commit, REFS_BLOB_PATH)?;
        let signature_bytes = Self::read_blob(repository, &commit, SIGNATURE_BLOB_PATH)?;
        let (refs, canonical) = self.parse_refs(&refs_bytes)?;
        let signature = RadicleSignature::try_from(signature_bytes.as_slice()).map_err(|_| {
            domain(
                "radicle_source_signature_format",
                "radicle_source.sigrefs",
                "Radicle signed-reference signature has an invalid shape",
            )
        })?;
        let verifier = VerifyingKey::try_from(&self.signer).map_err(|_| {
            domain(
                "radicle_source_signer_key",
                "radicle_source.signer",
                "Radicle signer identity cannot verify signatures",
            )
        })?;
        verifier.verify(&canonical, &signature).map_err(|_| {
            domain(
                "radicle_source_signature",
                "radicle_source.sigrefs",
                "Radicle signed-reference signature verification failed",
            )
        })?;

        let identity_root_commit = refs.get(IDENTITY_ROOT_REFERENCE).copied().ok_or_else(|| {
            domain(
                "radicle_source_identity_root",
                "radicle_source.sigrefs",
                "Radicle signed references do not contain an identity root",
            )
        })?;
        let identity_commit = repository
            .find_commit(identity_root_commit)
            .map_err(git_failure)?;
        let identity_tree = identity_commit.tree().map_err(git_failure)?;
        let identity_entry = identity_tree
            .get_path(Path::new(IDENTITY_DOCUMENT_PATH))
            .map_err(git_failure)?;
        if identity_entry.kind() != Some(ObjectType::Blob) {
            return Err(domain(
                "radicle_source_identity_document",
                "radicle_source.identity",
                "Radicle identity document is not a blob",
            ));
        }
        let observed_rid = RepoId::from(identity_entry.id());
        if observed_rid != self.repository_id {
            return Err(domain(
                "radicle_source_identity_substitution",
                "radicle_source.identity",
                "Radicle identity root differs from the requested RID",
            ));
        }

        let declared_parent = refs.get(SIGREFS_PARENT_REFERENCE).copied();
        match (parent, declared_parent) {
            (None, None) => {}
            (Some(actual), Some(declared)) if actual == declared => {}
            _ => {
                return Err(domain(
                    "radicle_source_sigrefs_parent",
                    "radicle_source.sigrefs",
                    "Radicle signed-reference parent does not match its commit history",
                ));
            }
        }

        let signed_revision = refs
            .get(&self.profile.request.signed_reference)
            .copied()
            .ok_or_else(|| {
                domain(
                    "radicle_source_signed_branch",
                    "radicle_source.reference",
                    "Requested branch is absent from the signed-reference map",
                )
            })?;
        if signed_revision != self.expected_revision {
            return Err(domain(
                "radicle_source_revision_substitution",
                "radicle_source.revision",
                "Signed branch revision differs from the exact request",
            ));
        }
        let namespaced_revision =
            Self::reference_target(repository, &self.namespaced_source_reference())?;
        if namespaced_revision != signed_revision {
            return Err(domain(
                "radicle_source_namespace_substitution",
                "radicle_source.reference",
                "Namespaced branch target differs from its signed revision",
            ));
        }
        repository
            .find_commit(signed_revision)
            .map_err(git_failure)?;
        Ok(VerifiedSignedRefs {
            sigrefs_commit,
            sigrefs_parent: parent,
            identity_root_commit,
            signed_revision,
            namespaced_reference,
        })
    }

    fn collect_tree(
        &self,
        repository: &Repository,
        tree: &Tree<'_>,
        prefix: &str,
        entries: &mut Vec<GitSnapshotEntry>,
        total_bytes: &mut u64,
    ) -> PortResult<()> {
        for entry in tree {
            let entry_count = u64::try_from(entries.len()).map_err(|_| {
                domain(
                    "radicle_source_entry_bound",
                    "radicle_source.snapshot",
                    "Snapshot entry count exceeds the supported range",
                )
            })?;
            if entry_count >= self.profile.request.limits.max_entries {
                return Err(domain(
                    "radicle_source_entry_bound",
                    "radicle_source.snapshot",
                    "Snapshot entry count exceeds the request bound",
                ));
            }
            let name = std::str::from_utf8(entry.name_bytes()).map_err(|_| {
                domain(
                    "radicle_source_path_encoding",
                    "radicle_source.snapshot",
                    "Snapshot path is not valid UTF-8",
                )
            })?;
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            };
            let path_bytes = u64::try_from(path.len()).map_err(|_| {
                domain(
                    "radicle_source_path_bound",
                    "radicle_source.snapshot",
                    "Snapshot path exceeds the supported range",
                )
            })?;
            if path_bytes > self.profile.request.limits.max_path_bytes {
                return Err(domain(
                    "radicle_source_path_bound",
                    "radicle_source.snapshot",
                    "Snapshot path exceeds the request bound",
                ));
            }
            match entry.kind() {
                Some(ObjectType::Tree) => {
                    let subtree = repository.find_tree(entry.id()).map_err(git_failure)?;
                    self.collect_tree(repository, &subtree, &path, entries, total_bytes)?;
                }
                Some(ObjectType::Blob) => {
                    let blob = repository.find_blob(entry.id()).map_err(git_failure)?;
                    let size = u64::try_from(blob.size()).map_err(|_| {
                        domain(
                            "radicle_source_file_bound",
                            "radicle_source.snapshot",
                            "Snapshot file size exceeds the supported range",
                        )
                    })?;
                    if size > self.profile.request.limits.max_file_bytes {
                        return Err(domain(
                            "radicle_source_file_bound",
                            "radicle_source.snapshot",
                            "Snapshot file exceeds the request bound",
                        ));
                    }
                    *total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
                        domain(
                            "radicle_source_total_bound",
                            "radicle_source.snapshot",
                            "Snapshot total byte count overflowed",
                        )
                    })?;
                    if *total_bytes > self.profile.request.limits.max_total_bytes {
                        return Err(domain(
                            "radicle_source_total_bound",
                            "radicle_source.snapshot",
                            "Snapshot bytes exceed the request bound",
                        ));
                    }
                    let kind = match entry.filemode() {
                        MODE_REGULAR => GitEntryKind::Regular,
                        MODE_EXECUTABLE => GitEntryKind::Executable,
                        MODE_SYMLINK => GitEntryKind::Symlink,
                        _ => GitEntryKind::Unsupported,
                    };
                    entries.push(GitSnapshotEntry {
                        path,
                        kind,
                        native_object: ExternalRef::new(
                            "radicle",
                            "git-object",
                            &entry.id().to_string(),
                        )?,
                        bytes: blob.content().to_vec(),
                    });
                }
                Some(ObjectType::Commit) if entry.filemode() == MODE_GITLINK => {
                    return Err(domain(
                        "radicle_source_gitlink",
                        "radicle_source.snapshot",
                        "Radicle source profile does not admit Git links",
                    ));
                }
                _ => {
                    return Err(domain(
                        "radicle_source_entry_kind",
                        "radicle_source.snapshot",
                        "Snapshot contains an unsupported Git entry kind",
                    ));
                }
            }
        }
        Ok(())
    }

    fn acquire_snapshot(
        &self,
        repository: &Repository,
        revision: Oid,
    ) -> PortResult<AcquiredSnapshot> {
        let commit = repository.find_commit(revision).map_err(git_failure)?;
        let tree = commit.tree().map_err(git_failure)?;
        let mut entries = Vec::new();
        let mut total_bytes = 0;
        self.collect_tree(repository, &tree, "", &mut entries, &mut total_bytes)?;
        let limits = GitSnapshotLimits {
            max_entries: usize::try_from(self.profile.request.limits.max_entries)
                .map_err(|_| platform_bound_failure())?,
            max_path_bytes: usize::try_from(self.profile.request.limits.max_path_bytes)
                .map_err(|_| platform_bound_failure())?,
            max_file_bytes: usize::try_from(self.profile.request.limits.max_file_bytes)
                .map_err(|_| platform_bound_failure())?,
            max_total_bytes: usize::try_from(self.profile.request.limits.max_total_bytes)
                .map_err(|_| platform_bound_failure())?,
        };
        let manifest =
            build_git_snapshot_manifest(&entries, &limits).map_err(PortFailure::Domain)?;
        let entry_count = u64::try_from(entries.len()).map_err(|_| platform_bound_failure())?;
        let canonical_tree_blake3 = blake3::hash(&manifest);
        Ok(AcquiredSnapshot {
            manifest,
            native_tree: tree.id(),
            entry_count,
            total_bytes,
            canonical_tree_blake3,
        })
    }

    fn execute(&self) -> PortResult<(RadicleSourceResult, Vec<u8>)> {
        let admitted = admit_source_profile(&self.profile)?;
        let repository = self.open()?;
        self.inspect_repository(&repository)?;
        let verified = self.verify_signed_refs(&repository)?;
        let snapshot = self.acquire_snapshot(&repository, verified.signed_revision)?;
        let observed_after = Self::reference_target(&repository, &verified.namespaced_reference)?;
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
        let verified = self.verify_signed_refs(&repository)?;
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

fn parse_oid(value: &str, field: &str) -> PortResult<Oid> {
    Oid::from_str(value).map_err(|_| {
        domain(
            "radicle_source_oid",
            field,
            "Radicle source identity is not a valid Git object identity",
        )
    })
}

fn platform_bound_failure() -> PortFailure {
    domain(
        "radicle_source_platform_bound",
        "radicle_source.limits",
        "Radicle source bound exceeds this platform",
    )
}

fn git_failure(_error: git2::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_source_git".to_string(),
        message: "Radicle source repository operation failed".to_string(),
    })
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
