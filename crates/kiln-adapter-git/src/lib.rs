use std::collections::BTreeSet;
use std::path::PathBuf;

use git2::{ObjectType, Oid, Repository, Tree};
use kiln_codec::identity_from_bytes;
use kiln_core::source::{
    GitEntryKind, GitObjectFormat, GitResolutionFacts, GitSnapshotEntry, GitSnapshotLimits,
    SourceCapability, SourceNativeFacts, SourceProviderProfile, SourceResolution,
    admit_git_resolution, admit_source_capability, build_git_snapshot_manifest,
    validate_resolution,
};
use kiln_core::types::{Alias, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, SourceRepositoryBackend,
    SourceRepositoryObservation, SourceRepositoryPort, SourceResolutionObservation,
};
use serde::{Deserialize, Serialize};

pub const PINNED_GIT2_SOURCE: &str = "crates.io:git2:0.20.2";
pub const PINNED_GIT2_LICENSE: &str = "MIT OR Apache-2.0";
pub const PINNED_LIBGIT2_SOURCE: &str = "crates.io:libgit2-sys:0.18.7+1.9.6";
pub const PINNED_LIBGIT2_LICENSE: &str = "GPL-2.0-only with libgit2 linking exception";
pub const PINNED_OBJECT_FORMAT: &str = "sha1";
pub const PINNED_REPOSITORY_FORMAT_VERSION: i64 = 0;
pub const PINNED_PLATFORM: &str = "linux";

const MODE_REGULAR: i32 = 0o100_644;
const MODE_EXECUTABLE: i32 = 0o100_755;
const MODE_SYMLINK: i32 = 0o120_000;
const MODE_GITLINK: i32 = 0o160_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitSourceProfile {
    pub id: ProviderProfileId,
    pub library_source: String,
    pub native_library_source: String,
    pub license: String,
    pub native_license: String,
    pub object_format: GitObjectFormat,
    pub repository_format_version: i64,
    pub platform: String,
    pub capabilities: BTreeSet<SourceCapability>,
    pub limits: GitSnapshotLimits,
    pub allow_network: bool,
    pub allow_credentials: bool,
    pub allow_mutation: bool,
    pub allow_signing: bool,
    pub allow_collaboration: bool,
    pub allow_gitlinks: bool,
    pub non_claims: Vec<String>,
}

impl GitSourceProfile {
    #[must_use]
    pub fn pinned_local(profile_id: ProviderProfileId, limits: GitSnapshotLimits) -> Self {
        Self {
            id: profile_id,
            library_source: PINNED_GIT2_SOURCE.to_string(),
            native_library_source: PINNED_LIBGIT2_SOURCE.to_string(),
            license: PINNED_GIT2_LICENSE.to_string(),
            native_license: PINNED_LIBGIT2_LICENSE.to_string(),
            object_format: GitObjectFormat::Sha1,
            repository_format_version: PINNED_REPOSITORY_FORMAT_VERSION,
            platform: PINNED_PLATFORM.to_string(),
            capabilities: BTreeSet::from([
                SourceCapability::Discover,
                SourceCapability::ResolveExact,
                SourceCapability::Acquire,
                SourceCapability::ObserveIdentity,
            ]),
            limits,
            allow_network: false,
            allow_credentials: false,
            allow_mutation: false,
            allow_signing: false,
            allow_collaboration: false,
            allow_gitlinks: false,
            non_claims: vec![
                "local Git observations do not prove source or signature trust".to_string(),
                "local Git observations do not prove complete history or remote availability"
                    .to_string(),
                "local Git observations do not grant forge or collaboration authority".to_string(),
            ],
        }
    }
}

// r[impl kiln.git.profile] r[impl kiln.git.capabilities] r[impl kiln.git.nonclaims]
pub fn admit_profile(profile: &GitSourceProfile) -> PortResult<SourceProviderProfile> {
    if profile.library_source != PINNED_GIT2_SOURCE
        || profile.native_library_source != PINNED_LIBGIT2_SOURCE
        || profile.license != PINNED_GIT2_LICENSE
        || profile.native_license != PINNED_LIBGIT2_LICENSE
        || profile.object_format != GitObjectFormat::Sha1
        || profile.repository_format_version != PINNED_REPOSITORY_FORMAT_VERSION
        || profile.platform != PINNED_PLATFORM
        || profile.limits.max_entries == 0
        || profile.limits.max_path_bytes == 0
        || profile.limits.max_file_bytes == 0
        || profile.limits.max_total_bytes == 0
        || profile.non_claims.is_empty()
    {
        return Err(Diagnostic::new(
            "git_profile",
            "git.profile",
            "Git source profile does not match the pinned local cohort",
        )
        .into());
    }
    if profile.allow_network
        || profile.allow_credentials
        || profile.allow_mutation
        || profile.allow_signing
        || profile.allow_collaboration
        || profile.allow_gitlinks
    {
        return Err(Diagnostic::new(
            "git_profile_authority",
            "git.profile.capabilities",
            "Git source profile grants authority outside bounded local reads",
        )
        .into());
    }
    let required = BTreeSet::from([
        SourceCapability::Discover,
        SourceCapability::ResolveExact,
        SourceCapability::Acquire,
        SourceCapability::ObserveIdentity,
    ]);
    if profile.capabilities != required {
        return Err(Diagnostic::new(
            "git_profile_capabilities",
            "git.profile.capabilities",
            "Git source profile capabilities differ from the read-only cohort",
        )
        .into());
    }
    let admitted = SourceProviderProfile {
        id: profile.id.clone(),
        native_system: ExternalRef::new("git", "source-provider", PINNED_GIT2_SOURCE)?,
        capabilities: required,
        identity_namespaces: BTreeSet::from([Alias::new("git-commit")?, Alias::new("git-tree")?]),
        immutable_resolution: true,
    };
    for capability in [
        SourceCapability::Discover,
        SourceCapability::ResolveExact,
        SourceCapability::Acquire,
        SourceCapability::ObserveIdentity,
    ] {
        admit_source_capability(&admitted, capability, &admitted.id)?;
    }
    Ok(admitted)
}

// r[impl kiln.git.conformance]
#[derive(Debug)]
pub struct LocalGitAdapter {
    repository_path: PathBuf,
    symbolic_reference: String,
    profile: GitSourceProfile,
}

impl LocalGitAdapter {
    pub fn new(
        repository_path: PathBuf,
        symbolic_reference: String,
        profile: GitSourceProfile,
    ) -> PortResult<Self> {
        let _admitted = admit_profile(&profile)?;
        if symbolic_reference.is_empty() || symbolic_reference.len() > profile.limits.max_path_bytes
        {
            return Err(Diagnostic::new(
                "git_reference_bound",
                "git.reference",
                "Git reference is empty or exceeds its named bound",
            )
            .into());
        }
        Ok(Self {
            repository_path,
            symbolic_reference,
            profile,
        })
    }

    fn open(&self) -> PortResult<Repository> {
        Repository::open(&self.repository_path).map_err(git_failure)
    }

    fn repository_identity(&self) -> PortResult<ExternalRef> {
        let canonical = self.repository_path.canonicalize().map_err(io_failure)?;
        let text = canonical.to_str().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "git_repository_path",
                "git.repository",
                "Git repository path is not valid UTF-8",
            ))
        })?;
        let identity = identity_from_bytes(text.as_bytes()).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "git_repository_identity",
                "git.repository",
                "Git repository identity could not be computed",
            ))
        })?;
        ExternalRef::new("git", "repository", identity.as_str()).map_err(PortFailure::Domain)
    }

    fn inspect_repository(&self, repository: &Repository) -> PortResult<RepositoryFeatures> {
        let config = repository.config().map_err(git_failure)?;
        let format_version = config
            .get_i64("core.repositoryformatversion")
            .unwrap_or(PINNED_REPOSITORY_FORMAT_VERSION);
        let object_format = match config.get_string("extensions.objectformat") {
            Ok(value) if value.eq_ignore_ascii_case(PINNED_OBJECT_FORMAT) => GitObjectFormat::Sha1,
            Ok(_) => GitObjectFormat::Other,
            Err(_) => GitObjectFormat::Sha1,
        };
        let partial_repository = config.get_string("extensions.partialclone").is_ok();
        let worktree_extension = config
            .get_bool("extensions.worktreeconfig")
            .unwrap_or(false);
        let replacement_objects = repository
            .references_glob("refs/replace/*")
            .map_err(git_failure)?
            .next()
            .is_some();
        let git_dir = repository.path();
        let alternate_object_database = git_dir.join("objects/info/alternates").exists();
        let features = RepositoryFeatures {
            format_version,
            object_format,
            partial_repository,
            worktree_extension,
            replacement_objects,
            alternate_object_database,
        };
        if features.format_version != self.profile.repository_format_version
            || features.object_format != self.profile.object_format
            || features.partial_repository
            || features.worktree_extension
            || features.replacement_objects
            || features.alternate_object_database
        {
            return Err(Diagnostic::new(
                "git_repository_feature",
                "git.repository_features",
                "Git repository requires a feature outside the selected profile",
            )
            .into());
        }
        Ok(features)
    }

    fn resolve_oid(&self, repository: &Repository, symbolic: &str) -> PortResult<Oid> {
        if let Ok(oid) = Oid::from_str(symbolic) {
            repository.find_commit(oid).map_err(git_failure)?;
            return Ok(oid);
        }
        let reference = repository.find_reference(symbolic).map_err(git_failure)?;
        reference
            .peel_to_commit()
            .map(|commit| commit.id())
            .map_err(git_failure)
    }

    fn collect_entries(
        &self,
        repository: &Repository,
        tree: &Tree<'_>,
    ) -> PortResult<Vec<GitSnapshotEntry>> {
        let mut entries = Vec::new();
        let mut total_bytes = 0usize;
        self.collect_tree(repository, tree, "", &mut entries, &mut total_bytes)?;
        Ok(entries)
    }

    fn collect_tree(
        &self,
        repository: &Repository,
        tree: &Tree<'_>,
        prefix: &str,
        entries: &mut Vec<GitSnapshotEntry>,
        total_bytes: &mut usize,
    ) -> PortResult<()> {
        for entry in tree {
            if entries.len() >= self.profile.limits.max_entries {
                return Err(Diagnostic::new(
                    "git_snapshot_entries",
                    "git.snapshot.entries",
                    "Git snapshot entry count exceeds its named bound",
                )
                .into());
            }
            let name = std::str::from_utf8(entry.name_bytes()).map_err(|_| {
                PortFailure::Domain(Diagnostic::new(
                    "git_snapshot_path",
                    "git.snapshot.path",
                    "Git snapshot path is not valid UTF-8",
                ))
            })?;
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            };
            if path.len() > self.profile.limits.max_path_bytes {
                return Err(Diagnostic::new(
                    "git_snapshot_path",
                    "git.snapshot.path",
                    "Git snapshot path exceeds its named bound",
                )
                .into());
            }
            match entry.kind() {
                Some(ObjectType::Tree) => {
                    let subtree = repository.find_tree(entry.id()).map_err(git_failure)?;
                    self.collect_tree(repository, &subtree, &path, entries, total_bytes)?;
                }
                Some(ObjectType::Blob) => {
                    let blob = repository.find_blob(entry.id()).map_err(git_failure)?;
                    if blob.size() > self.profile.limits.max_file_bytes {
                        return Err(Diagnostic::new(
                            "git_snapshot_file_bound",
                            "git.snapshot.bytes",
                            "Git snapshot file exceeds its named bound",
                        )
                        .into());
                    }
                    *total_bytes = total_bytes.checked_add(blob.size()).ok_or_else(|| {
                        PortFailure::Domain(Diagnostic::new(
                            "git_snapshot_total_bound",
                            "git.snapshot.bytes",
                            "Git snapshot total byte count overflowed",
                        ))
                    })?;
                    if *total_bytes > self.profile.limits.max_total_bytes {
                        return Err(Diagnostic::new(
                            "git_snapshot_total_bound",
                            "git.snapshot.bytes",
                            "Git snapshot total bytes exceed the named bound",
                        )
                        .into());
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
                        native_object: ExternalRef::new("git", "object", &entry.id().to_string())?,
                        bytes: blob.content().to_vec(),
                    });
                }
                Some(ObjectType::Commit) if entry.filemode() == MODE_GITLINK => {
                    entries.push(GitSnapshotEntry {
                        path,
                        kind: GitEntryKind::Gitlink,
                        native_object: ExternalRef::new("git", "object", &entry.id().to_string())?,
                        bytes: Vec::new(),
                    });
                }
                _ => {
                    return Err(Diagnostic::new(
                        "git_snapshot_entry_kind",
                        "git.snapshot.kind",
                        "Git snapshot entry kind is not supported by this profile",
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    fn acquire_exact(&self, commit_id: &ExternalRef) -> PortResult<AcquiredSnapshot> {
        let repository = self.open()?;
        let features = self.inspect_repository(&repository)?;
        let observed_after = self.resolve_oid(&repository, &self.symbolic_reference)?;
        if observed_after.to_string() != commit_id.native_id.as_str() {
            return Err(Diagnostic::new(
                "git_reference_moved",
                "git.reference",
                "Git reference changed during source acquisition",
            )
            .into());
        }
        let oid = Oid::from_str(commit_id.native_id.as_str()).map_err(git_failure)?;
        let commit = repository.find_commit(oid).map_err(git_failure)?;
        let signature_observed = repository.extract_signature(&oid, None).is_ok();
        let tree = commit.tree().map_err(git_failure)?;
        let entries = self.collect_entries(&repository, &tree)?;
        let manifest = build_git_snapshot_manifest(&entries, &self.profile.limits)?;
        Ok(AcquiredSnapshot {
            manifest,
            native_tree: ExternalRef::new("git", "tree", &tree.id().to_string())?,
            observed_after: ExternalRef::new("git", "commit", &observed_after.to_string())?,
            signature_observed,
            features,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositoryFeatures {
    format_version: i64,
    object_format: GitObjectFormat,
    partial_repository: bool,
    worktree_extension: bool,
    replacement_objects: bool,
    alternate_object_database: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcquiredSnapshot {
    manifest: Vec<u8>,
    native_tree: ExternalRef,
    observed_after: ExternalRef,
    signature_observed: bool,
    features: RepositoryFeatures,
}

impl SourceRepositoryPort for LocalGitAdapter {
    fn observe(&self) -> PortResult<SourceRepositoryObservation> {
        let repository = self.open()?;
        let _features = self.inspect_repository(&repository)?;
        let repository_identity = self.repository_identity()?;
        Ok(SourceRepositoryObservation {
            repository: repository_identity,
            backend: SourceRepositoryBackend::Git,
            operation: None,
        })
    }

    fn resolve(&self, symbolic: &ExternalRef) -> PortResult<SourceResolutionObservation> {
        if symbolic.provider_namespace.as_str() != "git"
            || symbolic.native_kind.as_str() != "reference"
            || symbolic.native_id.as_str() != self.symbolic_reference
        {
            return Err(Diagnostic::new(
                "git_reference_substitution",
                "git.reference",
                "Git resolution request differs from the configured reference",
            )
            .into());
        }
        let repository = self.open()?;
        let _features = self.inspect_repository(&repository)?;
        let oid = self.resolve_oid(&repository, &self.symbolic_reference)?;
        Ok(SourceResolutionObservation {
            symbolic: symbolic.clone(),
            native_change: None,
            exact_revisions: vec![ExternalRef::new("git", "commit", &oid.to_string())?],
            alias: self
                .symbolic_reference
                .strip_prefix("refs/heads/")
                .map(Alias::new)
                .transpose()?,
            conflicted_alias: false,
            divergent_change: false,
        })
    }

    fn acquire_canonical_manifest(&self, commit: &ExternalRef) -> PortResult<Vec<u8>> {
        self.acquire_exact(commit).map(|snapshot| snapshot.manifest)
    }
}

// r[impl kiln.git.resolution] r[impl kiln.git.snapshot]
pub fn resolve_and_acquire(
    adapter: &LocalGitAdapter,
    provider_profile: ProviderProfileId,
) -> PortResult<SourceResolution> {
    if provider_profile != adapter.profile.id {
        return Err(Diagnostic::new(
            "git_provider_substitution",
            "git.profile.id",
            "Git provider differs from the selected profile",
        )
        .into());
    }
    let admitted_profile = admit_profile(&adapter.profile)?;
    admit_source_capability(
        &admitted_profile,
        SourceCapability::ResolveExact,
        &provider_profile,
    )?;
    let observation = adapter.observe()?;
    let symbolic = ExternalRef::new("git", "reference", adapter.symbolic_reference.as_str())?;
    let resolved = adapter.resolve(&symbolic)?;
    let exact_commit = resolved.exact_revisions.first().cloned().ok_or_else(|| {
        PortFailure::Domain(Diagnostic::new(
            "git_revision_missing",
            "git.revision",
            "Git reference did not resolve to an exact commit",
        ))
    })?;
    if resolved.exact_revisions.len() != 1 {
        return Err(Diagnostic::new(
            "git_revision_ambiguous",
            "git.revision",
            "Git reference did not resolve to one exact commit",
        )
        .into());
    }
    let acquired = adapter.acquire_exact(&exact_commit)?;
    let canonical_tree = identity_from_bytes(&acquired.manifest).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "git_tree_identity",
            "git.snapshot",
            "Git canonical tree identity could not be computed",
        ))
    })?;
    let facts = GitResolutionFacts {
        repository: observation.repository.clone(),
        symbolic_reference: symbolic.clone(),
        exact_commit: exact_commit.clone(),
        native_tree: acquired.native_tree,
        observed_ref_before: exact_commit.clone(),
        observed_ref_after: acquired.observed_after,
        object_format: acquired.features.object_format,
        partial_repository: acquired.features.partial_repository,
        replacement_objects: acquired.features.replacement_objects,
        alternate_object_database: acquired.features.alternate_object_database,
        signature_observed: acquired.signature_observed,
    };
    admit_git_resolution(&facts)?;
    let resolution = SourceResolution {
        source_provider: provider_profile,
        repository: observation.repository,
        symbolic_reference: symbolic,
        exact_revision: exact_commit,
        canonical_tree,
        native_facts: SourceNativeFacts::Git(facts),
    };
    validate_resolution(&resolution)?;
    Ok(resolution)
}

fn git_failure(error: git2::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "git_infrastructure".to_string(),
        message: error.message().to_string(),
    })
}

fn io_failure(error: std::io::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "git_io".to_string(),
        message: error.to_string(),
    })
}

#[must_use]
pub fn profile_has_forbidden_authority(profile: &GitSourceProfile) -> bool {
    profile.allow_network
        || profile.allow_credentials
        || profile.allow_mutation
        || profile.allow_signing
        || profile.allow_collaboration
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use git2::{Repository, Signature};

    use super::*;

    const CONTENT_HEX_LENGTH: usize = 64;
    const MAX_ENTRIES: usize = 64;
    const MAX_PATH_BYTES: usize = 256;
    const MAX_FILE_BYTES: usize = 4096;
    const MAX_TOTAL_BYTES: usize = 16384;
    const FILE_MODE: i32 = MODE_REGULAR;
    const EXECUTABLE_MODE: i32 = MODE_EXECUTABLE;
    const SYMLINK_MODE: i32 = MODE_SYMLINK;
    const OID_DIRECTORY_PREFIX_BYTES: usize = 2;

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn limits() -> GitSnapshotLimits {
        GitSnapshotLimits {
            max_entries: MAX_ENTRIES,
            max_path_bytes: MAX_PATH_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
        }
    }

    fn profile() -> GitSourceProfile {
        GitSourceProfile::pinned_local(
            ProviderProfileId::new("git-source-v1").unwrap_or_else(|_| unreachable!()),
            limits(),
        )
    }

    fn fixture_path(label: &str) -> PathBuf {
        let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "kiln-git-adapter-{}-{label}-{sequence}",
            std::process::id()
        ))
    }

    fn create_commit(repository: &Repository, message: &str, content: &[u8]) -> Oid {
        let blob = repository.blob(content).unwrap_or_else(|_| unreachable!());
        let executable = repository
            .blob(b"#!/bin/sh\nexit 0\n")
            .unwrap_or_else(|_| unreachable!());
        let symlink = repository
            .blob(b"file.txt")
            .unwrap_or_else(|_| unreachable!());
        let mut builder = repository
            .treebuilder(None)
            .unwrap_or_else(|_| unreachable!());
        builder
            .insert("file.txt", blob, FILE_MODE)
            .unwrap_or_else(|_| unreachable!());
        builder
            .insert("script", executable, EXECUTABLE_MODE)
            .unwrap_or_else(|_| unreachable!());
        builder
            .insert("link", symlink, SYMLINK_MODE)
            .unwrap_or_else(|_| unreachable!());
        let tree_id = builder.write().unwrap_or_else(|_| unreachable!());
        let tree = repository
            .find_tree(tree_id)
            .unwrap_or_else(|_| unreachable!());
        let signature = Signature::now("Kiln Fixture", "kiln@example.invalid")
            .unwrap_or_else(|_| unreachable!());
        let parents = repository
            .find_reference("refs/heads/main")
            .ok()
            .and_then(|reference| reference.peel_to_commit().ok())
            .into_iter()
            .collect::<Vec<_>>();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .unwrap_or_else(|_| unreachable!())
    }

    fn fixture(label: &str) -> (PathBuf, Repository, Oid) {
        let path = fixture_path(label);
        let repository = Repository::init(&path).unwrap_or_else(|_| unreachable!());
        let commit = create_commit(&repository, "initial", b"content");
        (path, repository, commit)
    }

    fn create_empty_commit(repository: &Repository) -> Oid {
        let builder = repository
            .treebuilder(None)
            .unwrap_or_else(|_| unreachable!());
        let tree_id = builder.write().unwrap_or_else(|_| unreachable!());
        let tree = repository
            .find_tree(tree_id)
            .unwrap_or_else(|_| unreachable!());
        let signature = Signature::now("Kiln Fixture", "kiln@example.invalid")
            .unwrap_or_else(|_| unreachable!());
        let parent = repository
            .find_reference("refs/heads/main")
            .and_then(|reference| reference.peel_to_commit())
            .unwrap_or_else(|_| unreachable!());
        repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "empty",
                &tree,
                &[&parent],
            )
            .unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.git.profile.scenario.match]
    #[test]
    fn pinned_profile_is_read_only() {
        let profile = profile();
        assert!(admit_profile(&profile).is_ok());
        assert!(!profile_has_forbidden_authority(&profile));
    }

    // r[verify kiln.git.profile.scenario.feature]
    #[test]
    fn profile_rejects_network_and_format_drift() {
        let mut invalid = profile();
        invalid.allow_network = true;
        assert!(admit_profile(&invalid).is_err());
        invalid.allow_network = false;
        invalid.library_source = "git2-unpinned".to_string();
        assert!(admit_profile(&invalid).is_err());
    }

    // r[verify kiln.git.resolution.scenario.stable]
    // r[verify kiln.git.snapshot.scenario.valid]
    // r[verify kiln.git.conformance.scenario.native]
    // r[verify kiln.git.nonclaims.scenario.scope]
    #[test]
    fn real_repository_resolves_and_acquires_repeatably() {
        let (path, _repository, commit) = fixture("stable");
        let adapter = LocalGitAdapter::new(path.clone(), "refs/heads/main".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        let first = resolve_and_acquire(&adapter, adapter.profile.id.clone());
        let second = resolve_and_acquire(&adapter, adapter.profile.id.clone());
        assert_eq!(first, second);
        assert!(first.is_ok_and(|resolution| {
            resolution.exact_revision.native_id.as_str() == commit.to_string()
                && resolution.canonical_tree.as_str().len() == CONTENT_HEX_LENGTH
        }));
        fs::remove_dir_all(path).unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn detached_commit_annotated_tag_and_empty_tree_are_supported() {
        let (path, repository, commit) = fixture("special");
        let detached = LocalGitAdapter::new(path.clone(), commit.to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        assert!(resolve_and_acquire(&detached, detached.profile.id.clone()).is_ok());

        let signature = Signature::now("Kiln Fixture", "kiln@example.invalid")
            .unwrap_or_else(|_| unreachable!());
        let object = repository
            .find_object(commit, Some(ObjectType::Commit))
            .unwrap_or_else(|_| unreachable!());
        repository
            .tag("v1", &object, &signature, "tag", false)
            .unwrap_or_else(|_| unreachable!());
        let tag = LocalGitAdapter::new(path.clone(), "refs/tags/v1".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        assert!(resolve_and_acquire(&tag, tag.profile.id.clone()).is_ok());

        let empty_commit = create_empty_commit(&repository);
        let empty = LocalGitAdapter::new(path.clone(), "refs/heads/main".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        let empty_resolution = resolve_and_acquire(&empty, empty.profile.id.clone());
        assert!(empty_resolution.is_ok_and(|resolution| {
            resolution.exact_revision.native_id.as_str() == empty_commit.to_string()
        }));
        drop(object);
        drop(repository);
        fs::remove_dir_all(path).unwrap_or_else(|_| unreachable!());
    }

    // r[verify kiln.git.resolution.scenario.moved]
    #[test]
    fn moved_reference_is_rejected_before_identity_return() {
        let (path, repository, commit) = fixture("moved");
        let adapter = LocalGitAdapter::new(path.clone(), "refs/heads/main".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        let old = ExternalRef::new("git", "commit", &commit.to_string())
            .unwrap_or_else(|_| unreachable!());
        let _new_commit = create_commit(&repository, "moved", b"new content");
        assert!(adapter.acquire_canonical_manifest(&old).is_err());
        drop(repository);
        fs::remove_dir_all(path).unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn missing_commit_object_fails_closed() {
        let (path, repository, commit) = fixture("missing-object");
        drop(repository);
        let oid = commit.to_string();
        let (directory, file) = oid.split_at(OID_DIRECTORY_PREFIX_BYTES);
        fs::remove_file(path.join(".git/objects").join(directory).join(file))
            .unwrap_or_else(|_| unreachable!());
        let adapter = LocalGitAdapter::new(path.clone(), "refs/heads/main".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        assert!(resolve_and_acquire(&adapter, adapter.profile.id.clone()).is_err());
        fs::remove_dir_all(path).unwrap_or_else(|_| unreachable!());
    }

    // r[verify kiln.git.snapshot.scenario.path]
    #[test]
    fn missing_repository_and_unsupported_features_fail_closed() {
        let missing = LocalGitAdapter::new(
            fixture_path("missing"),
            "refs/heads/main".to_string(),
            profile(),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(missing.observe().is_err());

        let (path, repository, _commit) = fixture("feature");
        let mut config = repository.config().unwrap_or_else(|_| unreachable!());
        config
            .set_bool("extensions.worktreeconfig", true)
            .unwrap_or_else(|_| unreachable!());
        let adapter = LocalGitAdapter::new(path.clone(), "refs/heads/main".to_string(), profile())
            .unwrap_or_else(|_| unreachable!());
        assert!(adapter.observe().is_err());
        drop(config);
        drop(repository);
        fs::remove_dir_all(path).unwrap_or_else(|_| unreachable!());
    }

    // r[verify kiln.git.capabilities.scenario.read]
    // r[verify kiln.git.capabilities.scenario.fetch]
    #[test]
    fn adapter_exposes_no_network_or_mutation_surface() {
        let profile = profile();
        assert!(!profile_has_forbidden_authority(&profile));
        assert!(!profile.capabilities.contains(&SourceCapability::Mutate));
        assert!(
            !profile
                .capabilities
                .contains(&SourceCapability::UseCredentials)
        );
        assert!(
            !profile
                .capabilities
                .contains(&SourceCapability::Collaborate)
        );

        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        for forbidden in [
            "Repository::clone",
            "repository.commit(",
            "repository.branch(",
            "repository.remote(",
            "remote.fetch(",
            "remote.push(",
            "RemoteCallbacks",
            "Cred::",
            "std::process",
            "std::net",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden Git adapter surface: {forbidden}"
            );
        }
    }
}
