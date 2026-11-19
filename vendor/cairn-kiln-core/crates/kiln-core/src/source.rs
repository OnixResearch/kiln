use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{Alias, ContentId, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SourceCapability {
    Discover,
    ResolveExact,
    Acquire,
    ObserveIdentity,
    Collaborate,
    ReceiveTrigger,
    PublishStatus,
    UseCredentials,
    Mutate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceProviderProfile {
    pub id: ProviderProfileId,
    pub native_system: ExternalRef,
    pub capabilities: BTreeSet<SourceCapability>,
    pub identity_namespaces: BTreeSet<Alias>,
    pub immutable_resolution: bool,
}

// r[impl kiln.git.capabilities]
pub fn admit_source_capability(
    profile: &SourceProviderProfile,
    capability: SourceCapability,
    expected_profile: &ProviderProfileId,
) -> CoreResult<()> {
    if &profile.id != expected_profile {
        return Err(Diagnostic::new(
            "source_provider_substitution",
            "source_provider.id",
            "source provider differs from the selected profile",
        ));
    }
    profile.native_system.validate()?;
    if !profile.capabilities.contains(&capability) {
        return Err(Diagnostic::new(
            "source_capability_missing",
            "source_provider.capabilities",
            "source provider does not declare the requested narrow capability",
        ));
    }
    if matches!(
        capability,
        SourceCapability::ResolveExact | SourceCapability::Acquire
    ) && !profile.immutable_resolution
    {
        return Err(Diagnostic::new(
            "moving_source_resolution",
            "source_provider.immutable_resolution",
            "source acquisition requires an immutable exact resolution",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceTriggerObservation {
    pub provider: ProviderProfileId,
    pub native_event: ExternalRef,
    pub actor: ExternalRef,
    pub exact_revision: ExternalRef,
    pub signature_observed: bool,
    pub signature_valid: bool,
    pub replayed: bool,
}

pub fn admit_source_trigger(
    profile: &SourceProviderProfile,
    trigger: &SourceTriggerObservation,
) -> CoreResult<()> {
    admit_source_capability(profile, SourceCapability::ReceiveTrigger, &trigger.provider)?;
    trigger.native_event.validate()?;
    trigger.actor.validate()?;
    trigger.exact_revision.validate()?;
    if trigger.replayed {
        return Err(Diagnostic::new(
            "replayed_source_trigger",
            "source_trigger.native_event",
            "source trigger was already admitted",
        ));
    }
    if trigger.signature_observed && !trigger.signature_valid {
        return Err(Diagnostic::new(
            "invalid_source_signature",
            "source_trigger.signature",
            "observed source trigger signature is invalid",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitObjectFormat {
    Sha1,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum GitEntryKind {
    Regular,
    Executable,
    Symlink,
    Gitlink,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitResolutionFacts {
    pub repository: ExternalRef,
    pub symbolic_reference: ExternalRef,
    pub exact_commit: ExternalRef,
    pub native_tree: ExternalRef,
    pub observed_ref_before: ExternalRef,
    pub observed_ref_after: ExternalRef,
    pub object_format: GitObjectFormat,
    pub partial_repository: bool,
    pub replacement_objects: bool,
    pub alternate_object_database: bool,
    pub signature_observed: bool,
}

// r[impl kiln.git.resolution] r[impl kiln.git.profile]
pub fn admit_git_resolution(facts: &GitResolutionFacts) -> CoreResult<()> {
    facts.repository.validate()?;
    facts.symbolic_reference.validate()?;
    facts.exact_commit.validate()?;
    facts.native_tree.validate()?;
    facts.observed_ref_before.validate()?;
    facts.observed_ref_after.validate()?;
    if facts.object_format != GitObjectFormat::Sha1 {
        return Err(Diagnostic::new(
            "git_object_format",
            "git.object_format",
            "Git object format is not in the selected local profile",
        ));
    }
    if facts.partial_repository || facts.replacement_objects || facts.alternate_object_database {
        return Err(Diagnostic::new(
            "git_repository_feature",
            "git.repository_features",
            "Git repository requires an unsupported source feature",
        ));
    }
    if facts.observed_ref_before != facts.exact_commit
        || facts.observed_ref_after != facts.exact_commit
    {
        return Err(Diagnostic::new(
            "git_reference_moved",
            "git.reference",
            "Git reference changed during source acquisition",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitSnapshotLimits {
    pub max_entries: usize,
    pub max_path_bytes: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitSnapshotEntry {
    pub path: String,
    pub kind: GitEntryKind,
    pub native_object: ExternalRef,
    pub bytes: Vec<u8>,
}

const GIT_SNAPSHOT_TAG: &[u8] = b"kiln.git.snapshot.v1\0";
const GIT_ENTRY_REGULAR: u8 = b'f';
const GIT_ENTRY_EXECUTABLE: u8 = b'x';
const GIT_ENTRY_SYMLINK: u8 = b'l';

fn append_framed(output: &mut Vec<u8>, bytes: &[u8]) -> CoreResult<()> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        Diagnostic::new(
            "git_snapshot_bound",
            "git.snapshot",
            "Git snapshot field length exceeds the framing range",
        )
    })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn validate_git_path(path: &str, max_path_bytes: usize) -> CoreResult<()> {
    if path.is_empty()
        || path.len() > max_path_bytes
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(Diagnostic::new(
            "git_snapshot_path",
            "git.snapshot.path",
            "Git snapshot path is unsafe or exceeds its named bound",
        ));
    }
    Ok(())
}

// r[impl kiln.git.snapshot]
pub fn build_git_snapshot_manifest(
    entries: &[GitSnapshotEntry],
    limits: &GitSnapshotLimits,
) -> CoreResult<Vec<u8>> {
    if entries.len() > limits.max_entries {
        return Err(Diagnostic::new(
            "git_snapshot_entries",
            "git.snapshot.entries",
            "Git snapshot entry count exceeds its named bound",
        ));
    }
    let mut ordered = entries.to_vec();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut previous_path: Option<&str> = None;
    let mut total_bytes = 0usize;
    let mut manifest = GIT_SNAPSHOT_TAG.to_vec();
    for entry in &ordered {
        validate_git_path(&entry.path, limits.max_path_bytes)?;
        if previous_path == Some(entry.path.as_str()) {
            return Err(Diagnostic::new(
                "git_snapshot_duplicate",
                "git.snapshot.path",
                "Git snapshot has a duplicate path",
            ));
        }
        previous_path = Some(entry.path.as_str());
        if entry.bytes.len() > limits.max_file_bytes {
            return Err(Diagnostic::new(
                "git_snapshot_file_bound",
                "git.snapshot.bytes",
                "Git snapshot file exceeds its named bound",
            ));
        }
        total_bytes = total_bytes.checked_add(entry.bytes.len()).ok_or_else(|| {
            Diagnostic::new(
                "git_snapshot_total_bound",
                "git.snapshot.bytes",
                "Git snapshot total byte count overflowed",
            )
        })?;
        if total_bytes > limits.max_total_bytes {
            return Err(Diagnostic::new(
                "git_snapshot_total_bound",
                "git.snapshot.bytes",
                "Git snapshot total bytes exceed the named bound",
            ));
        }
        let kind = match entry.kind {
            GitEntryKind::Regular => GIT_ENTRY_REGULAR,
            GitEntryKind::Executable => GIT_ENTRY_EXECUTABLE,
            GitEntryKind::Symlink => GIT_ENTRY_SYMLINK,
            GitEntryKind::Gitlink | GitEntryKind::Unsupported => {
                return Err(Diagnostic::new(
                    "git_snapshot_entry_kind",
                    "git.snapshot.kind",
                    "Git snapshot entry kind is not supported by this profile",
                ));
            }
        };
        entry.native_object.validate()?;
        manifest.push(kind);
        append_framed(&mut manifest, entry.path.as_bytes())?;
        append_framed(&mut manifest, &entry.bytes)?;
    }
    Ok(manifest)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PijulEntryKind {
    Regular,
    Executable,
    Directory,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulSnapshotLimits {
    pub max_changes: usize,
    pub max_entries: usize,
    pub max_path_bytes: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulSnapshotEntry {
    pub path: String,
    pub kind: PijulEntryKind,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulFacts {
    pub provider_profile: ProviderProfileId,
    pub repository: ExternalRef,
    pub channel: ExternalRef,
    pub state: ExternalRef,
    pub observed_state_before: ExternalRef,
    pub observed_state_after: ExternalRef,
    pub change_sequence: Vec<ExternalRef>,
    pub change_bound: usize,
    pub format_cohort: ExternalRef,
    pub observation_identity: ContentId,
}

const PIJUL_SNAPSHOT_TAG: &[u8] = b"kiln.pijul.snapshot.v1\0";
const PIJUL_OBSERVATION_TAG: &[u8] = b"kiln.pijul.observation.v1\0";
const PIJUL_ENTRY_REGULAR: u8 = b'f';
const PIJUL_ENTRY_EXECUTABLE: u8 = b'x';
const PIJUL_ENTRY_DIRECTORY: u8 = b'd';

fn append_pijul_framed(output: &mut Vec<u8>, bytes: &[u8]) -> CoreResult<()> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        Diagnostic::new(
            "pijul_snapshot_bound",
            "pijul.snapshot",
            "Pijul snapshot field length exceeds the framing range",
        )
    })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn validate_pijul_path(path: &str, max_path_bytes: usize) -> CoreResult<()> {
    if path.is_empty()
        || path.len() > max_path_bytes
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(Diagnostic::new(
            "pijul_snapshot_path",
            "pijul.snapshot.path",
            "Pijul snapshot path is unsafe or exceeds its named bound",
        ));
    }
    Ok(())
}

// r[impl kiln.source.pijul_snapshot]
pub fn build_pijul_snapshot_manifest(
    entries: &[PijulSnapshotEntry],
    limits: &PijulSnapshotLimits,
) -> CoreResult<Vec<u8>> {
    if limits.max_changes == 0
        || limits.max_entries == 0
        || limits.max_path_bytes == 0
        || limits.max_file_bytes == 0
        || limits.max_total_bytes == 0
    {
        return Err(Diagnostic::new(
            "pijul_snapshot_zero_bound",
            "pijul.snapshot.limits",
            "Pijul snapshot limits must all be positive",
        ));
    }
    if entries.len() > limits.max_entries {
        return Err(Diagnostic::new(
            "pijul_snapshot_entries",
            "pijul.snapshot.entries",
            "Pijul snapshot entry count exceeds its named bound",
        ));
    }
    let mut ordered = entries.to_vec();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut previous_path: Option<&str> = None;
    let mut total_bytes = 0usize;
    let mut manifest = PIJUL_SNAPSHOT_TAG.to_vec();
    for entry in &ordered {
        validate_pijul_path(&entry.path, limits.max_path_bytes)?;
        if previous_path == Some(entry.path.as_str()) {
            return Err(Diagnostic::new(
                "pijul_snapshot_duplicate",
                "pijul.snapshot.path",
                "Pijul snapshot has a duplicate path",
            ));
        }
        previous_path = Some(entry.path.as_str());
        if entry.bytes.len() > limits.max_file_bytes {
            return Err(Diagnostic::new(
                "pijul_snapshot_file_bound",
                "pijul.snapshot.bytes",
                "Pijul snapshot file exceeds its named bound",
            ));
        }
        total_bytes = total_bytes.checked_add(entry.bytes.len()).ok_or_else(|| {
            Diagnostic::new(
                "pijul_snapshot_total_bound",
                "pijul.snapshot.bytes",
                "Pijul snapshot total byte count overflowed",
            )
        })?;
        if total_bytes > limits.max_total_bytes {
            return Err(Diagnostic::new(
                "pijul_snapshot_total_bound",
                "pijul.snapshot.bytes",
                "Pijul snapshot total bytes exceed the named bound",
            ));
        }
        let kind = match entry.kind {
            PijulEntryKind::Regular => PIJUL_ENTRY_REGULAR,
            PijulEntryKind::Executable => PIJUL_ENTRY_EXECUTABLE,
            PijulEntryKind::Directory => {
                if !entry.bytes.is_empty() {
                    return Err(Diagnostic::new(
                        "pijul_snapshot_directory_bytes",
                        "pijul.snapshot.bytes",
                        "Pijul directory entry contains file bytes",
                    ));
                }
                PIJUL_ENTRY_DIRECTORY
            }
            PijulEntryKind::Unsupported => {
                return Err(Diagnostic::new(
                    "pijul_snapshot_entry_kind",
                    "pijul.snapshot.kind",
                    "Pijul snapshot entry kind is not supported by this profile",
                ));
            }
        };
        manifest.push(kind);
        append_pijul_framed(&mut manifest, entry.path.as_bytes())?;
        append_pijul_framed(&mut manifest, &entry.bytes)?;
    }
    Ok(manifest)
}

fn validate_pijul_reference(reference: &ExternalRef, kind: &str, path: &str) -> CoreResult<()> {
    reference.validate()?;
    if reference.provider_namespace.as_str() != "pijul" || reference.native_kind.as_str() != kind {
        return Err(Diagnostic::new(
            "pijul_identity_namespace",
            path,
            "Pijul fact uses the wrong provider namespace or native kind",
        ));
    }
    Ok(())
}

// r[impl kiln.source.pijul_state_identity]
pub fn compute_pijul_observation_identity(facts: &PijulFacts) -> CoreResult<ContentId> {
    let mut framed = PIJUL_OBSERVATION_TAG.to_vec();
    append_pijul_framed(&mut framed, facts.provider_profile.as_str().as_bytes())?;
    for fact in [
        &facts.repository,
        &facts.channel,
        &facts.state,
        &facts.observed_state_before,
        &facts.observed_state_after,
        &facts.format_cohort,
    ] {
        append_pijul_framed(&mut framed, fact.provider_namespace.as_str().as_bytes())?;
        append_pijul_framed(&mut framed, fact.native_kind.as_str().as_bytes())?;
        append_pijul_framed(&mut framed, fact.native_id.as_str().as_bytes())?;
    }
    for change in &facts.change_sequence {
        append_pijul_framed(&mut framed, change.native_id.as_str().as_bytes())?;
    }
    append_pijul_framed(&mut framed, &facts.change_bound.to_be_bytes())?;
    let digest = blake3::hash(&framed).to_hex();
    ContentId::from_blake3_hex(digest.as_str())
}

// r[impl kiln.source.pijul_state_identity]
pub fn admit_pijul_facts(facts: &PijulFacts) -> CoreResult<()> {
    validate_pijul_reference(&facts.repository, "repository", "pijul.repository")?;
    validate_pijul_reference(&facts.channel, "channel", "pijul.channel")?;
    validate_pijul_reference(&facts.state, "state", "pijul.state")?;
    validate_pijul_reference(
        &facts.observed_state_before,
        "state",
        "pijul.observed_state_before",
    )?;
    validate_pijul_reference(
        &facts.observed_state_after,
        "state",
        "pijul.observed_state_after",
    )?;
    validate_pijul_reference(&facts.format_cohort, "format-cohort", "pijul.format_cohort")?;
    if facts.change_bound == 0 || facts.change_sequence.len() > facts.change_bound {
        return Err(Diagnostic::new(
            "pijul_change_bound",
            "pijul.change_sequence",
            "Pijul change sequence exceeds its named bound",
        ));
    }
    let mut changes = BTreeSet::new();
    for change in &facts.change_sequence {
        validate_pijul_reference(change, "change", "pijul.change_sequence")?;
        if !changes.insert(change.clone()) {
            return Err(Diagnostic::new(
                "pijul_duplicate_change",
                "pijul.change_sequence",
                "Pijul change sequence contains a duplicate identity",
            ));
        }
    }
    if facts.state != facts.observed_state_before || facts.state != facts.observed_state_after {
        return Err(Diagnostic::new(
            "pijul_state_changed",
            "pijul.state",
            "Pijul channel state changed during snapshot acquisition",
        ));
    }
    if compute_pijul_observation_identity(facts)? != facts.observation_identity {
        return Err(Diagnostic::new(
            "pijul_observation_identity",
            "pijul.observation_identity",
            "Pijul observation identity does not bind the exact native facts",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JujutsuBackend {
    Git,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JujutsuFacts {
    pub backend: JujutsuBackend,
    pub operation_identity: ExternalRef,
    pub change_id: ExternalRef,
    pub bookmark: Option<Alias>,
    pub resolved_commits: Vec<ExternalRef>,
    pub conflicted_bookmark: bool,
    pub divergent_change: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceNativeFacts {
    Git(GitResolutionFacts),
    Jujutsu(JujutsuFacts),
    Pijul(PijulFacts),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceResolution {
    pub source_provider: ProviderProfileId,
    pub repository: ExternalRef,
    pub symbolic_reference: ExternalRef,
    pub exact_revision: ExternalRef,
    pub canonical_tree: ContentId,
    pub native_facts: SourceNativeFacts,
}

pub fn validate_resolution(resolution: &SourceResolution) -> CoreResult<()> {
    resolution.repository.validate()?;
    resolution.symbolic_reference.validate()?;
    resolution.exact_revision.validate()?;
    match &resolution.native_facts {
        SourceNativeFacts::Git(facts) => {
            admit_git_resolution(facts)?;
            if facts.repository != resolution.repository
                || facts.symbolic_reference != resolution.symbolic_reference
                || facts.exact_commit != resolution.exact_revision
            {
                return Err(Diagnostic::new(
                    "git_resolution_facts",
                    "source_resolution.native_facts",
                    "Git native facts differ from the common source resolution",
                ));
            }
        }
        SourceNativeFacts::Jujutsu(facts) => {
            validate_jujutsu(facts, &resolution.exact_revision)?;
        }
        SourceNativeFacts::Pijul(facts) => {
            admit_pijul_facts(facts)?;
            if facts.provider_profile != resolution.source_provider
                || facts.repository != resolution.repository
                || facts.channel != resolution.symbolic_reference
                || facts.state != resolution.exact_revision
            {
                return Err(Diagnostic::new(
                    "pijul_resolution_facts",
                    "source_resolution.native_facts",
                    "Pijul native facts differ from the common source resolution",
                ));
            }
            if resolution.exact_revision.native_kind.as_str() != "state" {
                return Err(Diagnostic::new(
                    "pijul_incomplete_state",
                    "source_resolution.exact_revision",
                    "One Pijul change identity cannot replace the complete channel state",
                ));
            }
        }
    }
    Ok(())
}

// r[impl kiln.providers.policy_effect_split]
pub fn admit_jujutsu_facts(facts: &JujutsuFacts) -> CoreResult<&ExternalRef> {
    if facts.backend != JujutsuBackend::Git {
        return Err(Diagnostic::new(
            "unsupported_backend",
            "jujutsu.backend",
            "Jujutsu backend is not admitted",
        ));
    }
    if facts.conflicted_bookmark {
        return Err(Diagnostic::new(
            "conflicted_bookmark",
            "jujutsu.bookmark",
            "Jujutsu bookmark is conflicted",
        ));
    }
    if facts.divergent_change {
        return Err(Diagnostic::new(
            "divergent_change",
            "jujutsu.change_id",
            "Jujutsu change is divergent",
        ));
    }
    if facts.resolved_commits.len() != 1 {
        return Err(Diagnostic::new(
            "ambiguous_revision",
            "jujutsu.resolved_commits",
            "Jujutsu reference must resolve to exactly one commit",
        ));
    }
    let Some(resolved) = facts.resolved_commits.first() else {
        return Err(Diagnostic::new(
            "missing_revision",
            "jujutsu.resolved_commits",
            "Jujutsu reference did not resolve",
        ));
    };
    Ok(resolved)
}

fn validate_jujutsu(facts: &JujutsuFacts, exact_revision: &ExternalRef) -> CoreResult<()> {
    let resolved = admit_jujutsu_facts(facts)?;
    if resolved != exact_revision {
        return Err(Diagnostic::new(
            "revision_mismatch",
            "jujutsu.resolved_commits",
            "resolved Jujutsu commit differs from the exact source revision",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposedChangeCandidate {
    pub base: SourceResolution,
    pub head: SourceResolution,
    pub strategy: Alias,
    pub result: SourceResolution,
}

pub fn validate_candidate(candidate: &ProposedChangeCandidate) -> CoreResult<()> {
    validate_resolution(&candidate.base)?;
    validate_resolution(&candidate.head)?;
    validate_resolution(&candidate.result)?;
    if candidate.base.source_provider != candidate.result.source_provider
        || candidate.head.source_provider != candidate.result.source_provider
    {
        return Err(Diagnostic::new(
            "provider_mismatch",
            "candidate.result",
            "candidate snapshots use different source providers",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MirrorDisagreementPolicy {
    Deny,
    PreferCanonical,
}

pub fn compare_mirrors(
    canonical: &SourceResolution,
    mirror: &SourceResolution,
    policy: MirrorDisagreementPolicy,
) -> CoreResult<bool> {
    validate_resolution(canonical)?;
    validate_resolution(mirror)?;
    if canonical.exact_revision == mirror.exact_revision
        && canonical.canonical_tree == mirror.canonical_tree
    {
        return Ok(true);
    }
    match policy {
        MirrorDisagreementPolicy::Deny => Err(Diagnostic::new(
            "mirror_disagreement",
            "source_mirror",
            "source mirrors disagree",
        )),
        MirrorDisagreementPolicy::PreferCanonical => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn external(namespace: &str, kind: &str, value: &str) -> ExternalRef {
        ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
    }

    fn tree() -> ContentId {
        ContentId::from_blake3_hex(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn admits_exact_jujutsu_commit() {
        let commit = external("jj", "commit", "abc");
        let resolution = SourceResolution {
            source_provider: ProviderProfileId::new("jj-source-v1")
                .unwrap_or_else(|_| unreachable!()),
            repository: external("jj", "repository", "repo"),
            symbolic_reference: external("jj", "bookmark", "main"),
            exact_revision: commit.clone(),
            canonical_tree: tree(),
            native_facts: SourceNativeFacts::Jujutsu(JujutsuFacts {
                backend: JujutsuBackend::Git,
                operation_identity: external("jj", "operation", "op"),
                change_id: external("jj", "change", "change"),
                bookmark: Some(Alias::new("main").unwrap_or_else(|_| unreachable!())),
                resolved_commits: alloc::vec![commit],
                conflicted_bookmark: false,
                divergent_change: false,
            }),
        };
        assert_eq!(validate_resolution(&resolution), Ok(()));
    }

    #[test]
    fn narrow_source_profile_does_not_gain_forge_authority() {
        let profile = SourceProviderProfile {
            id: ProviderProfileId::new("jj-source-v1").unwrap_or_else(|_| unreachable!()),
            native_system: external("jj", "source-provider", "git-backend"),
            capabilities: BTreeSet::from([
                SourceCapability::Discover,
                SourceCapability::ResolveExact,
                SourceCapability::Acquire,
                SourceCapability::ObserveIdentity,
            ]),
            identity_namespaces: BTreeSet::from([
                Alias::new("jj-commit").unwrap_or_else(|_| unreachable!()),
                Alias::new("jj-change").unwrap_or_else(|_| unreachable!()),
            ]),
            immutable_resolution: true,
        };
        assert_eq!(
            admit_source_capability(&profile, SourceCapability::Acquire, &profile.id),
            Ok(())
        );
        assert!(
            admit_source_capability(&profile, SourceCapability::Collaborate, &profile.id).is_err()
        );
    }

    #[test]
    fn native_source_profiles_keep_distinct_identity_namespaces() {
        for (provider_name, namespace) in [
            ("git-source-v1", "git-commit"),
            ("pijul-source-v1", "pijul-change"),
            ("jj-source-v1", "jj-change"),
            ("github-collaboration-v1", "github-pull-request"),
            ("radicle-collaboration-v1", "radicle-patch"),
        ] {
            let profile = SourceProviderProfile {
                id: ProviderProfileId::new(provider_name).unwrap_or_else(|_| unreachable!()),
                native_system: external(provider_name, "provider", "fixture"),
                capabilities: BTreeSet::from([SourceCapability::ObserveIdentity]),
                identity_namespaces: BTreeSet::from([
                    Alias::new(namespace).unwrap_or_else(|_| unreachable!())
                ]),
                immutable_resolution: true,
            };
            assert!(
                profile
                    .identity_namespaces
                    .iter()
                    .any(|value| value.as_str() == namespace)
            );
        }
    }

    #[test]
    fn replayed_or_invalidly_signed_trigger_fails_closed() {
        let profile = SourceProviderProfile {
            id: ProviderProfileId::new("radicle-trigger-v1").unwrap_or_else(|_| unreachable!()),
            native_system: external("radicle", "trigger-provider", "node"),
            capabilities: BTreeSet::from([SourceCapability::ReceiveTrigger]),
            identity_namespaces: BTreeSet::new(),
            immutable_resolution: true,
        };
        let mut trigger = SourceTriggerObservation {
            provider: profile.id.clone(),
            native_event: external("radicle", "event", "event"),
            actor: external("radicle", "actor", "did:key:actor"),
            exact_revision: external("radicle", "revision", "revision"),
            signature_observed: true,
            signature_valid: false,
            replayed: false,
        };
        assert!(admit_source_trigger(&profile, &trigger).is_err());
        trigger.signature_valid = true;
        trigger.replayed = true;
        assert!(admit_source_trigger(&profile, &trigger).is_err());
    }

    const GIT_MAX_ENTRIES: usize = 8;
    const GIT_MAX_PATH_BYTES: usize = 128;
    const GIT_MAX_FILE_BYTES: usize = 1024;
    const GIT_MAX_TOTAL_BYTES: usize = 4096;

    fn git_limits() -> GitSnapshotLimits {
        GitSnapshotLimits {
            max_entries: GIT_MAX_ENTRIES,
            max_path_bytes: GIT_MAX_PATH_BYTES,
            max_file_bytes: GIT_MAX_FILE_BYTES,
            max_total_bytes: GIT_MAX_TOTAL_BYTES,
        }
    }

    fn git_entry(path: &str, kind: GitEntryKind, value: &str) -> GitSnapshotEntry {
        GitSnapshotEntry {
            path: path.into(),
            kind,
            native_object: external("git", "object", value),
            bytes: value.as_bytes().to_vec(),
        }
    }

    // r[verify kiln.git.snapshot.scenario.valid]
    #[test]
    fn git_snapshot_manifest_is_deterministic() {
        let entries = alloc::vec![
            git_entry("script", GitEntryKind::Executable, "second"),
            git_entry("link", GitEntryKind::Symlink, "first"),
            git_entry("file", GitEntryKind::Regular, "third"),
        ];
        let mut reversed = entries.clone();
        reversed.reverse();
        assert_eq!(
            build_git_snapshot_manifest(&entries, &git_limits()),
            build_git_snapshot_manifest(&reversed, &git_limits())
        );
        let mut equal_tree_with_other_native_objects = entries.clone();
        for entry in &mut equal_tree_with_other_native_objects {
            entry.native_object = external("git", "object", "different-history");
        }
        assert_eq!(
            build_git_snapshot_manifest(&entries, &git_limits()),
            build_git_snapshot_manifest(&equal_tree_with_other_native_objects, &git_limits())
        );
    }

    // r[verify kiln.git.snapshot.scenario.path]
    #[test]
    fn git_snapshot_rejects_unsafe_duplicate_and_gitlink_entries() {
        let unsafe_entry = git_entry("../secret", GitEntryKind::Regular, "unsafe");
        assert!(build_git_snapshot_manifest(&[unsafe_entry], &git_limits()).is_err());
        let duplicate = git_entry("same", GitEntryKind::Regular, "duplicate");
        assert!(
            build_git_snapshot_manifest(&[duplicate.clone(), duplicate], &git_limits()).is_err()
        );
        let gitlink = git_entry("module", GitEntryKind::Gitlink, "gitlink");
        assert!(build_git_snapshot_manifest(&[gitlink], &git_limits()).is_err());
        let mut small_limit = git_limits();
        small_limit.max_file_bytes = 1;
        let oversized = git_entry("large", GitEntryKind::Regular, "oversized");
        assert!(build_git_snapshot_manifest(&[oversized], &small_limit).is_err());
    }

    // r[verify kiln.git.resolution.scenario.stable]
    // r[verify kiln.git.profile.scenario.match]
    #[test]
    fn admits_stable_local_git_resolution() {
        let commit = external("git", "commit", "commit");
        let facts = GitResolutionFacts {
            repository: external("git", "repository", "repo"),
            symbolic_reference: external("git", "reference", "refs/heads/main"),
            exact_commit: commit.clone(),
            native_tree: external("git", "tree", "tree"),
            observed_ref_before: commit.clone(),
            observed_ref_after: commit,
            object_format: GitObjectFormat::Sha1,
            partial_repository: false,
            replacement_objects: false,
            alternate_object_database: false,
            signature_observed: false,
        };
        assert!(admit_git_resolution(&facts).is_ok());
    }

    // r[verify kiln.git.resolution.scenario.moved]
    // r[verify kiln.git.profile.scenario.feature]
    #[test]
    fn rejects_moved_or_unsupported_git_resolution() {
        let commit = external("git", "commit", "commit");
        let mut facts = GitResolutionFacts {
            repository: external("git", "repository", "repo"),
            symbolic_reference: external("git", "reference", "refs/heads/main"),
            exact_commit: commit.clone(),
            native_tree: external("git", "tree", "tree"),
            observed_ref_before: commit,
            observed_ref_after: external("git", "commit", "moved"),
            object_format: GitObjectFormat::Sha1,
            partial_repository: false,
            replacement_objects: false,
            alternate_object_database: false,
            signature_observed: false,
        };
        assert!(admit_git_resolution(&facts).is_err());
        facts.observed_ref_after = facts.exact_commit.clone();
        facts.partial_repository = true;
        assert!(admit_git_resolution(&facts).is_err());
    }

    // r[verify kiln.git.capabilities.scenario.read]
    // r[verify kiln.git.capabilities.scenario.fetch]
    #[test]
    fn local_git_profile_has_no_network_or_mutation_authority() {
        let profile = SourceProviderProfile {
            id: ProviderProfileId::new("git-source-v1").unwrap_or_else(|_| unreachable!()),
            native_system: external("git", "source-provider", "libgit2"),
            capabilities: BTreeSet::from([
                SourceCapability::Discover,
                SourceCapability::ResolveExact,
                SourceCapability::Acquire,
                SourceCapability::ObserveIdentity,
            ]),
            identity_namespaces: BTreeSet::from([
                Alias::new("git-commit").unwrap_or_else(|_| unreachable!()),
                Alias::new("git-tree").unwrap_or_else(|_| unreachable!()),
            ]),
            immutable_resolution: true,
        };
        assert!(admit_source_capability(&profile, SourceCapability::Acquire, &profile.id).is_ok());
        for denied in [
            SourceCapability::UseCredentials,
            SourceCapability::Mutate,
            SourceCapability::Collaborate,
        ] {
            assert!(admit_source_capability(&profile, denied, &profile.id).is_err());
        }
    }

    const PIJUL_MAX_CHANGES: usize = 8;
    const PIJUL_MAX_ENTRIES: usize = 8;
    const PIJUL_MAX_PATH_BYTES: usize = 128;
    const PIJUL_MAX_FILE_BYTES: usize = 1024;
    const PIJUL_MAX_TOTAL_BYTES: usize = 4096;

    fn pijul_limits() -> PijulSnapshotLimits {
        PijulSnapshotLimits {
            max_changes: PIJUL_MAX_CHANGES,
            max_entries: PIJUL_MAX_ENTRIES,
            max_path_bytes: PIJUL_MAX_PATH_BYTES,
            max_file_bytes: PIJUL_MAX_FILE_BYTES,
            max_total_bytes: PIJUL_MAX_TOTAL_BYTES,
        }
    }

    fn pijul_facts() -> PijulFacts {
        let mut facts = PijulFacts {
            provider_profile: ProviderProfileId::new("pijul-source-v1")
                .unwrap_or_else(|_| unreachable!()),
            repository: external("pijul", "repository", "repo"),
            channel: external("pijul", "channel", "main"),
            state: external("pijul", "state", "state"),
            observed_state_before: external("pijul", "state", "state"),
            observed_state_after: external("pijul", "state", "state"),
            change_sequence: alloc::vec![external("pijul", "change", "change")],
            change_bound: PIJUL_MAX_CHANGES,
            format_cohort: external("pijul", "format-cohort", "sanakirja-pristine-v1"),
            observation_identity: tree(),
        };
        facts.observation_identity =
            compute_pijul_observation_identity(&facts).unwrap_or_else(|_| unreachable!());
        facts
    }

    // r[verify kiln.source.pijul_state_identity.scenario.stable]
    // r[verify kiln.source.pijul_state_identity.scenario.moving]
    // r[verify kiln.source.pijul_state_identity.scenario.incomplete]
    #[test]
    fn pijul_facts_bind_complete_stable_state() {
        let facts = pijul_facts();
        assert!(admit_pijul_facts(&facts).is_ok());
        let mut changed = facts.clone();
        changed.observed_state_after = external("pijul", "state", "moved");
        assert!(admit_pijul_facts(&changed).is_err());
        let mut incomplete = facts;
        incomplete.state = external("pijul", "change", "change");
        assert!(admit_pijul_facts(&incomplete).is_err());

        let facts = pijul_facts();
        let mixed_provider = SourceResolution {
            source_provider: ProviderProfileId::new("jj-source-v1")
                .unwrap_or_else(|_| unreachable!()),
            repository: facts.repository.clone(),
            symbolic_reference: facts.channel.clone(),
            exact_revision: facts.state.clone(),
            canonical_tree: tree(),
            native_facts: SourceNativeFacts::Pijul(facts),
        };
        assert!(validate_resolution(&mixed_provider).is_err());
    }

    // r[verify kiln.source.pijul_snapshot.scenario.acquire]
    // r[verify kiln.source.pijul_snapshot.scenario.entry]
    // r[verify kiln.source.pijul_snapshot.scenario.history]
    #[test]
    fn pijul_snapshot_is_deterministic_and_rejects_invalid_entries() {
        let entries = alloc::vec![
            PijulSnapshotEntry {
                path: "dir".into(),
                kind: PijulEntryKind::Directory,
                bytes: Vec::new(),
            },
            PijulSnapshotEntry {
                path: "dir/file".into(),
                kind: PijulEntryKind::Regular,
                bytes: b"content".to_vec(),
            },
        ];
        let mut reversed = entries.clone();
        reversed.reverse();
        assert_eq!(
            build_pijul_snapshot_manifest(&entries, &pijul_limits()),
            build_pijul_snapshot_manifest(&reversed, &pijul_limits())
        );
        let unsafe_entry = PijulSnapshotEntry {
            path: "../secret".into(),
            kind: PijulEntryKind::Regular,
            bytes: Vec::new(),
        };
        assert!(build_pijul_snapshot_manifest(&[unsafe_entry], &pijul_limits()).is_err());
        let duplicate = PijulSnapshotEntry {
            path: "dir".into(),
            kind: PijulEntryKind::Directory,
            bytes: Vec::new(),
        };
        assert!(
            build_pijul_snapshot_manifest(&[duplicate.clone(), duplicate], &pijul_limits())
                .is_err()
        );
        let unsupported = PijulSnapshotEntry {
            path: "special".into(),
            kind: PijulEntryKind::Unsupported,
            bytes: Vec::new(),
        };
        assert!(build_pijul_snapshot_manifest(&[unsupported], &pijul_limits()).is_err());
        let mut zero_bound = pijul_limits();
        zero_bound.max_entries = 0;
        assert!(build_pijul_snapshot_manifest(&entries, &zero_bound).is_err());
    }

    #[test]
    fn rejects_ambiguous_jujutsu_revision() {
        let first = external("jj", "commit", "a");
        let second = external("jj", "commit", "b");
        let facts = JujutsuFacts {
            backend: JujutsuBackend::Git,
            operation_identity: external("jj", "operation", "op"),
            change_id: external("jj", "change", "change"),
            bookmark: None,
            resolved_commits: alloc::vec![first.clone(), second],
            conflicted_bookmark: false,
            divergent_change: true,
        };
        assert!(validate_jujutsu(&facts, &first).is_err());
    }
}
