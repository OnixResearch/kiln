//! Lattice v1 byte-snapshot receipt, or a herdr interactive-agent commit
//! receipt -> independently materialized exact Git source.
//! No Lattice runtime is linked and worker Git metadata is never consulted.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use git2::{IndexEntry, IndexTime, Oid, Repository, Signature, Time};
use kiln_core::implementation::{
    CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff, EvidenceKind, EvidenceOutcome,
    INTERACTIVE_PROVIDER_PROFILE, ImplementationAttempt, ImplementationDispatchPlan,
    ImplementationWork, MAX_CHANGED_PATHS, acceptance_digest, admit_candidate_handoff,
    admit_dispatch_plan,
};
use kiln_core::types::{ContentId, ExternalRef};
use serde::{Deserialize, Serialize};

use super::{VerificationProfile, VerifierAuthority, digest, source, text};

const MAX_SNAPSHOT_ENTRIES: usize = 50_000;
const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 2 * 1024 * 1024;
const LATTICE_CANDIDATE_RECEIPT_SCHEMA: &str = "lattice.candidate-receipt.v1";
const LATTICE_UNCHANGED_OBSERVATION_SCHEMA: &str = "lattice.unchanged-candidate-observation.v1";
/// Receipt the fleet shell writes after exporting an interactive agent's commit.
pub const HERDR_CANDIDATE_RECEIPT_SCHEMA: &str = "kiln.herdr.candidate-receipt.v1";
/// A full SHA-1 Git object id; abbreviated ids are refused so the reported
/// commit can never resolve ambiguously in the authority repository.
const GIT_OBJECT_ID_HEX_LENGTH: usize = 40;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostCandidateReceipt {
    schema: String,
    work_id: String,
    attempt_id: String,
    fence: String,
    workspace_id: String,
    base_identity: String,
    candidate_identity: Option<String>,
    diff_identity: Option<String>,
    changed_paths: Vec<String>,
    disposition: String,
    blockers: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HerdrCandidateDisposition {
    Produced,
    Unchanged,
}

/// Receipt for an interactive agent's handoff commit, admitted only for plans
/// with the `herdr-interactive` provider profile. `candidate_root` must be an
/// isolated export of exactly `candidate_commit`'s tree from the authority
/// repository; materialization refuses any other tree.
// The fence is a number here (the Lattice receipt carries it as text): this
// schema is Kiln-owned, so it takes the ledger's own type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerdrCandidateReceipt {
    pub schema: String,
    pub work_id: String,
    pub attempt_id: String,
    pub fence: u64,
    pub workspace_id: String,
    pub disposition: HerdrCandidateDisposition,
    pub blockers: Vec<String>,
    /// The agent's full Git commit id, fetched into the authority repository.
    pub candidate_commit: String,
    /// The fleet session identity that reported the handoff.
    pub session: ContentId,
}

impl HerdrCandidateReceipt {
    /// The receipt for the exact work, attempt, fence and retained workspace.
    pub fn for_attempt(
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        dispatch: &ImplementationDispatchPlan,
        disposition: HerdrCandidateDisposition,
        candidate_commit: &str,
        session: &ContentId,
    ) -> Result<Self, String> {
        Ok(Self {
            schema: HERDR_CANDIDATE_RECEIPT_SCHEMA.into(),
            work_id: work.identity().map_err(text)?.as_str().into(),
            attempt_id: attempt.identity.as_str().into(),
            fence: attempt.fence,
            workspace_id: dispatch.workspace.identity.as_str().into(),
            disposition,
            blockers: Vec::new(),
            candidate_commit: candidate_commit.into(),
            session: session.clone(),
        })
    }
}

/// A host receipt admitted for the retained plan's provider profile.
#[derive(Debug)]
enum AdmittedReceipt {
    Lattice(HostCandidateReceipt),
    Herdr(HerdrCandidateReceipt),
}

impl AdmittedReceipt {
    fn unchanged(&self) -> bool {
        match self {
            Self::Lattice(receipt) => receipt.disposition == "unchanged",
            Self::Herdr(receipt) => receipt.disposition == HerdrCandidateDisposition::Unchanged,
        }
    }
}

/// Admit exactly the receipt kind the retained plan's profile produces, bound
/// to the exact work, attempt, fence and workspace. A herdr receipt is refused
/// for any other profile and a Lattice receipt for the herdr profile.
// r[impl kiln.fleet.capture.scenario.receipt]
fn admit_receipt(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    dispatch: &ImplementationDispatchPlan,
    host_receipt: &[u8],
) -> Result<AdmittedReceipt, String> {
    if host_receipt.len() > MAX_RECEIPT_BYTES {
        return Err("host candidate receipt exceeds bound".into());
    }
    let work_id = work.identity().map_err(text)?;
    if dispatch.provider_profile.as_str() == INTERACTIVE_PROVIDER_PROFILE {
        let receipt: HerdrCandidateReceipt = serde_json::from_slice(host_receipt).map_err(text)?;
        if receipt.schema != HERDR_CANDIDATE_RECEIPT_SCHEMA
            || !receipt.blockers.is_empty()
            || receipt.work_id != work_id.as_str()
            || receipt.attempt_id != attempt.identity.as_str()
            || receipt.fence != attempt.fence
            || receipt.workspace_id != dispatch.workspace.identity.as_str()
            || receipt.session.validate().is_err()
            || receipt.candidate_commit.len() != GIT_OBJECT_ID_HEX_LENGTH
            || !receipt
                .candidate_commit
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(
                "herdr receipt is not for the exact work/attempt/fence/workspace and commit".into(),
            );
        }
        return Ok(AdmittedReceipt::Herdr(receipt));
    }
    let receipt: HostCandidateReceipt = serde_json::from_slice(host_receipt).map_err(text)?;
    let unchanged = receipt.disposition == "unchanged";
    let expected_schema = if unchanged {
        LATTICE_UNCHANGED_OBSERVATION_SCHEMA
    } else {
        LATTICE_CANDIDATE_RECEIPT_SCHEMA
    };
    if receipt.schema != expected_schema
        || (!unchanged && receipt.disposition != "produced")
        || !receipt.blockers.is_empty()
        || receipt.work_id != work_id.as_str()
        || receipt.attempt_id != attempt.identity.as_str()
        || receipt.fence != attempt.fence.to_string()
        || receipt.workspace_id != dispatch.workspace.identity.as_str()
    {
        return Err("host receipt is not produced for exact work/attempt/fence/workspace".into());
    }
    Ok(AdmittedReceipt::Lattice(receipt))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFact {
    length: u64,
    hash: [u8; 32],
    object: Oid,
    mode: u32,
}
type Snapshot = BTreeMap<String, FileFact>;

/// Export an exact Git commit tree from `repository_root` directly from the
/// object database into `destination`, without consulting `.gitattributes`
/// (`export-ignore`, `export-subst`, or smudge filters) or `tar`.
///
/// Preserves exact blob bytes and supported regular-file modes (`0o644` and
/// `0o755`) while enforcing the same path, entry-count, byte-count, and
/// unsupported-entry (symlink/submodule/special) bounds as materialization.
// r[impl kiln.fleet.capture]
pub fn export_commit_tree(
    repository_root: &Path,
    commit_id: &str,
    destination: &Path,
) -> Result<(), String> {
    use std::io::Write as _;
    let repository = Repository::open_bare(repository_root)
        .or_else(|_| Repository::open(repository_root))
        .map_err(text)?;
    let oid = Oid::from_str(commit_id).map_err(text)?;
    let commit = repository.find_commit(oid).map_err(text)?;
    let tree = commit.tree().map_err(text)?;
    let mut files = Snapshot::new();
    let mut total = 0_u64;
    collect_baseline(&repository, &tree, "", &mut files, &mut total)?;
    for (relative_path, fact) in &files {
        let target = destination.join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(text)?;
        }
        let blob = repository.find_blob(fact.object).map_err(text)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(text)?;
        file.write_all(blob.content()).map_err(text)?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = if fact.mode == 0o100755 { 0o755 } else { 0o644 };
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).map_err(text)?;
        }
    }
    Ok(())
}

/// Acquire bytes from a host-owned retained workspace after worker termination,
/// verify the native host receipt, and materialize real source in the enrolled
/// repository ODB without reading the worker's .git, index, refs or config.
///
/// The caller must enforce current lease/terminal-worker evidence in its durable
/// ledger before this operation and before registering the resulting handoff.
/// Source mutations here are local ODB/internal retention refs only, never a
/// target branch, remote publication or candidate verification.
// r[impl kiln.implementation.verification]
pub fn materialize_candidate(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    dispatch: &ImplementationDispatchPlan,
    profile: &VerificationProfile,
    authority: &VerifierAuthority,
    candidate_root: &Path,
    host_receipt: &[u8],
) -> Result<CandidateHandoff, String> {
    let plan = admit_dispatch_plan(work, attempt, dispatch).map_err(text)?;
    if attempt
        .dispatch
        .as_ref()
        .is_none_or(|reservation| reservation.plan != plan)
        || profile.identity()? != work.verification_profile
        || profile.repository != work.repository
        || profile.toolchain != profile.derived_toolchain_identity()?
        || profile.environment_identity != profile.derived_environment_identity()?
    {
        return Err("capture differs from retained dispatch or admitted verifier profile".into());
    }
    source::admit_authority(authority, work)?;
    materialize_admitted_candidate(
        work,
        attempt,
        dispatch,
        profile,
        authority,
        candidate_root,
        host_receipt,
    )
}

fn materialize_admitted_candidate(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    dispatch: &ImplementationDispatchPlan,
    profile: &VerificationProfile,
    authority: &VerifierAuthority,
    candidate_root: &Path,
    host_receipt: &[u8],
) -> Result<CandidateHandoff, String> {
    if !candidate_root.is_absolute()
        || fs::canonicalize(candidate_root).map_err(text)? != candidate_root
        || candidate_root.starts_with(&authority.repository_root)
        || candidate_root.starts_with(&authority.scratch_root)
    {
        return Err(
            "candidate source must be an isolated retained workspace, not verifier source/evidence"
                .into(),
        );
    }
    let receipt = admit_receipt(work, attempt, dispatch, host_receipt)?;
    let unchanged = receipt.unchanged();
    let base_ref = work
        .base
        .as_ref()
        .ok_or("capture requires exact baseline commit")?;
    if base_ref.provider_namespace.as_str() != "git" || base_ref.native_kind.as_str() != "commit" {
        return Err("candidate materialization supports exact Git baselines only".into());
    }
    let repository = Repository::open_bare(&authority.repository_root)
        .or_else(|_| Repository::open(&authority.repository_root))
        .map_err(text)?;
    let base_id = Oid::from_str(base_ref.native_id.as_str()).map_err(text)?;
    let base = repository.find_commit(base_id).map_err(text)?;
    let mut baseline = BTreeMap::new();
    let mut baseline_bytes = 0;
    collect_baseline(
        &repository,
        &base.tree().map_err(text)?,
        "",
        &mut baseline,
        &mut baseline_bytes,
    )?;
    let candidate = capture_workspace(&repository, candidate_root)?;
    // Byte-only native receipts cannot describe Unix mode-only changes. Reuse
    // the producer's explicit refusal, not a silently lossy mode interpretation.
    if candidate.iter().any(|(path, file)| {
        baseline
            .get(path)
            .map_or(file.mode != 0o100644, |old| old.mode != file.mode)
    }) {
        return Err("native byte receipt cannot admit file-mode changes".into());
    }
    let changed: Vec<_> = baseline
        .keys()
        .chain(candidate.keys())
        .filter(|path| match (baseline.get(*path), candidate.get(*path)) {
            (Some(left), Some(right)) => left.hash != right.hash || left.length != right.length,
            (None, None) => false,
            _ => true,
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if changed.is_empty() != unchanged || changed.len() > MAX_CHANGED_PATHS {
        return Err(
            "host snapshot/diff/complete paths differ from independently captured source".into(),
        );
    }
    if let AdmittedReceipt::Lattice(receipt) = &receipt
        && (receipt.changed_paths != changed
            || receipt.base_identity != tree_identity(&baseline)
            || receipt.candidate_identity.as_deref() != Some(tree_identity(&candidate).as_str())
            || receipt.diff_identity.as_deref()
                != Some(diff_identity(&baseline, &candidate, &changed).as_str()))
    {
        return Err(
            "host snapshot/diff/complete paths differ from independently captured source".into(),
        );
    }
    if unchanged
        && (!profile.allow_already_satisfied
            || work.terminal_policy
                != kiln_core::implementation::TerminalPolicy::VerifiedOrAlreadySatisfied)
    {
        return Err(
            "unchanged source has no explicit independently verified terminal policy".into(),
        );
    }
    // Build a private in-memory index. No candidate-controlled index, attributes,
    // filters, hooks or executable is interpreted when writing real Git objects.
    let mut index = git2::Index::new().map_err(text)?;
    for (path, file) in &candidate {
        index
            .add(&IndexEntry {
                ctime: IndexTime::new(0, 0),
                mtime: IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: file.mode,
                uid: 0,
                gid: 0,
                file_size: file.length as u32,
                id: file.object,
                flags: 0,
                flags_extended: 0,
                path: path.as_bytes().to_vec(),
            })
            .map_err(text)?;
    }
    let tree_id = index.write_tree_to(&repository).map_err(text)?;
    // The export must be exactly the reported agent commit's tree, as the
    // authority repository holds it after the fleet shell's fetch.
    if let AdmittedReceipt::Herdr(receipt) = &receipt {
        let reported = Oid::from_str(&receipt.candidate_commit).map_err(text)?;
        if repository.find_commit(reported).map_err(text)?.tree_id() != tree_id {
            return Err("exported candidate tree differs from the reported agent commit".into());
        }
    }
    let receipt_hash = digest(host_receipt);
    let commit_id = if unchanged {
        if tree_id != base.tree_id() {
            return Err("unchanged receipt substituted baseline tree".into());
        }
        base_id
    } else {
        let tree = repository.find_tree(tree_id).map_err(text)?;
        let signature = Signature::new("Kiln verifier", "verifier@kiln.invalid", &Time::new(0, 0))
            .map_err(text)?;
        let message = format!(
            "Kiln implementation candidate\n\nwork {}\nattempt {}\nfence {}\nhost-receipt {}\n",
            work.identity().map_err(text)?.as_str(),
            attempt.identity.as_str(),
            attempt.fence,
            receipt_hash
        );
        repository
            .commit(None, &signature, &signature, &message, &tree, &[&base])
            .map_err(text)?
    };
    // Retain unreachable materialization under a private, non-target namespace
    // so the verifier's later exact-object fetch can acquire it after restart.
    let retention = format!(
        "refs/kiln/implementation-candidates/{}",
        attempt.identity.as_str()
    );
    match repository.find_reference(&retention) {
        Ok(existing) if existing.target() == Some(commit_id) => {}
        Ok(_) => return Err("attempt already retained a different materialized candidate".into()),
        Err(error) if error.code() == git2::ErrorCode::NotFound => {
            repository
                .reference(
                    &retention,
                    commit_id,
                    false,
                    "retain exact implementation candidate",
                )
                .map_err(text)?;
        }
        Err(error) => return Err(text(error)),
    }
    let capture_directory = authority
        .scratch_root
        .join(format!("capture-{}", attempt.identity.as_str()));
    match fs::create_dir(&capture_directory) {
        Ok(()) => {
            fs::write(capture_directory.join("host-receipt.json"), host_receipt).map_err(text)?
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if super::bounded_file(
                &capture_directory.join("host-receipt.json"),
                MAX_RECEIPT_BYTES as u64,
            )? != host_receipt
            {
                return Err("capture replay substituted host receipt".into());
            }
        }
        Err(error) => return Err(text(error)),
    }
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.into(),
        work_identity: work.identity().map_err(text)?,
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: ExternalRef::new("git", "commit", &commit_id.to_string()).map_err(text)?,
        tree: ExternalRef::new("git", "tree", &tree_id.to_string()).map_err(text)?,
        changed_paths: changed,
        acceptance_digest: acceptance_digest(work).map_err(text)?,
        verification_profile: profile.identity()?,
        toolchain: profile.toolchain.clone(),
        environment: profile.environment_identity.clone(),
        evidence: vec![CandidateEvidence {
            identity: ContentId::from_blake3_hex(receipt_hash.clone()).map_err(text)?,
            kind: EvidenceKind::Artifact,
            outcome: EvidenceOutcome::Unknown,
            reference: ExternalRef::new("kiln", "host-candidate-receipt", &receipt_hash)
                .map_err(text)?,
        }],
    };
    admit_candidate_handoff(work, attempt, &handoff).map_err(text)?;
    fs::write(
        capture_directory.join("handoff.json"),
        serde_json::to_vec_pretty(&handoff).map_err(text)?,
    )
    .map_err(text)?;
    Ok(handoff)
}

fn collect_baseline(
    repository: &Repository,
    tree: &git2::Tree<'_>,
    prefix: &str,
    files: &mut Snapshot,
    total: &mut u64,
) -> Result<(), String> {
    for entry in tree.iter() {
        let name = entry.name().map_err(|_| "non UTF8 baseline path")?;
        let path = if prefix.is_empty() {
            name.into()
        } else {
            format!("{prefix}/{name}")
        };
        validate_path(&path)?;
        if entry.kind() == Some(git2::ObjectType::Tree) {
            collect_baseline(
                repository,
                &repository.find_tree(entry.id()).map_err(text)?,
                &path,
                files,
                total,
            )?;
        } else {
            if !matches!(entry.filemode(), 0o100644 | 0o100755) {
                return Err(
                    "symlink/submodule/special baseline unsupported by native byte receipt".into(),
                );
            }
            let blob = repository.find_blob(entry.id()).map_err(text)?;
            *total = total
                .checked_add(blob.size() as u64)
                .ok_or("baseline byte count overflow")?;
            if *total > MAX_SNAPSHOT_BYTES || files.len() >= MAX_SNAPSHOT_ENTRIES {
                return Err("baseline snapshot exceeds bounds".into());
            }
            files.insert(
                path,
                FileFact {
                    length: blob.size() as u64,
                    hash: *blake3::hash(blob.content()).as_bytes(),
                    object: blob.id(),
                    mode: entry.filemode() as u32,
                },
            );
        }
    }
    Ok(())
}

fn capture_workspace(repository: &Repository, root: &Path) -> Result<Snapshot, String> {
    let mut files = Snapshot::new();
    let mut total = 0_u64;
    let mut entries = 0_usize;
    let mut pending = vec![(root.to_path_buf(), PathBuf::new())];
    while let Some((directory, relative)) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(text)? {
            let entry = entry.map_err(text)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("non UTF8 candidate path")?;
            if relative.as_os_str().is_empty() && matches!(name, ".git" | ".jj") {
                continue;
            }
            entries += 1;
            if entries > MAX_SNAPSHOT_ENTRIES {
                return Err("candidate entry count exceeds native bound".into());
            }
            let path = relative.join(name);
            let label = path.to_str().ok_or("non UTF8 candidate path")?;
            validate_path(label)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(text)?;
            if metadata.file_type().is_symlink() {
                return Err("candidate symlink refused".into());
            }
            if metadata.is_dir() {
                pending.push((entry.path(), path));
                continue;
            }
            if !metadata.is_file() {
                return Err("candidate special file refused".into());
            }
            let mode = capture_mode(&metadata)?;
            let remaining = MAX_SNAPSHOT_BYTES - total;
            if metadata.len() > remaining {
                return Err("candidate snapshot exceeds byte bound".into());
            }
            let file = fs::File::open(entry.path()).map_err(text)?;
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            file.take(remaining + 1)
                .read_to_end(&mut bytes)
                .map_err(text)?;
            total = total
                .checked_add(bytes.len() as u64)
                .ok_or("candidate byte count overflow")?;
            if total > MAX_SNAPSHOT_BYTES {
                return Err("candidate grew beyond byte bound".into());
            }
            let fact = FileFact {
                length: bytes.len() as u64,
                hash: *blake3::hash(&bytes).as_bytes(),
                object: repository.blob(&bytes).map_err(text)?,
                mode,
            };
            if files.insert(label.into(), fact).is_some() {
                return Err("duplicate candidate path".into());
            }
        }
    }
    Ok(files)
}

// Exact native lattice.candidate-receipt.v1 framing from src/candidate.rs;
// identities remain producer-native BLAKE3, not substituted Git tree IDs.
fn tree_identity(snapshot: &Snapshot) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(b"lattice-candidate-tree-v1");
    length(&mut hash, snapshot.len() as u64);
    for (path, file) in snapshot {
        length(&mut hash, path.len() as u64);
        hash.update(path.as_bytes());
        length(&mut hash, file.length);
        hash.update(&file.hash);
    }
    hash.finalize().to_hex().to_string()
}
fn diff_identity(base: &Snapshot, candidate: &Snapshot, paths: &[String]) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(b"lattice-candidate-diff-v1");
    length(&mut hash, paths.len() as u64);
    for path in paths {
        length(&mut hash, path.len() as u64);
        hash.update(path.as_bytes());
        optional_file(&mut hash, base.get(path));
        optional_file(&mut hash, candidate.get(path));
    }
    hash.finalize().to_hex().to_string()
}
fn optional_file(hash: &mut blake3::Hasher, file: Option<&FileFact>) {
    if let Some(file) = file {
        hash.update(b"present");
        length(hash, file.length);
        hash.update(&file.hash);
    } else {
        hash.update(b"absent");
    }
}
fn length(hash: &mut blake3::Hasher, value: u64) {
    hash.update(&value.to_le_bytes());
}
fn validate_path(path: &str) -> Result<(), String> {
    if path.len() > 256 || !super::safe_relative(path) {
        return Err("unsafe or oversized native candidate path".into());
    }
    Ok(())
}

#[cfg(unix)]
fn capture_mode(metadata: &fs::Metadata) -> Result<u32, String> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        return Err("candidate hardlink refused".into());
    }
    match metadata.mode() & 0o7777 {
        0o644 => Ok(0o100644),
        0o755 => Ok(0o100755),
        _ => Err("candidate mode unsupported by native byte receipt".into()),
    }
}
#[cfg(not(unix))]
fn capture_mode(_metadata: &fs::Metadata) -> Result<u32, String> {
    Err("native candidate materialization supports Unix modes only".into())
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
