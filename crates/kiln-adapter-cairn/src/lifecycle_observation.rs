//! Read-only reconciliation of native Cairn lifecycle source already reviewed
//! in the candidate. Native receipt syntax is not mutation truth: every declared
//! post-state blob is independently read from the accepted Git tree.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use cairn_core::{adapt_cairn_sync_archive_receipt, canonical_json_hash};
use git2::{ErrorCode, ObjectType, Oid, Repository, Tree};
use kiln_core::implementation::{PublicationRecord, WorkSourceKind};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::publication_effects::{
    CairnLifecycleAction, CairnLifecycleBinding, CairnLifecycleObservation,
};
use serde_json::Value;

const MAX_RECEIPT_BYTES: usize = 2 * 1024 * 1024;
const MAX_MANIFEST_ENTRIES: usize = 4096;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_OBSERVED_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// Both first dispatch and crash reconciliation call this same read-only
/// consumer. `authoritative_report_path` is retained controller evidence, not a
/// new report to be evaluated. Its native hash must equal the accepted artifact.
// r[impl kiln.publication.atomicity]
pub fn observe_reviewed_lifecycle(
    repository: &Path,
    publication: &PublicationRecord,
    binding: &CairnLifecycleBinding,
    receipt_path: &str,
    authoritative_report_path: &Path,
) -> Result<CairnLifecycleObservation, String> {
    let integration = publication
        .integration
        .as_ref()
        .ok_or("lifecycle requires retained exact integration")?;
    if binding.source.kind != WorkSourceKind::CairnTask
        || binding.candidate != publication.plan.candidate
        || binding.handoff != publication.plan.handoff
        || binding.verification != publication.plan.verification
        || integration.candidate != binding.candidate
        || integration.mapping.candidate_tree != publication.plan.tree
        || integration.mapping.landed_tree != publication.plan.tree
    {
        return Err("lifecycle outer candidate/attempt binding differs".into());
    }
    observe_lifecycle_at_tree(
        repository,
        AcceptedSource {
            candidate: &publication.plan.candidate,
            accepted: &integration.merge_commit,
            tree: &publication.plan.tree,
            repository: &publication.plan.repository,
        },
        binding,
        receipt_path,
        authoritative_report_path,
    )
}

/// Consume independently retained native-Git receipt data without manufacturing a PR record.
pub fn observe_native_git_lifecycle(
    repository: &Path,
    native: &kiln_core::implementation::NativeGitReceipt,
    binding: &CairnLifecycleBinding,
    receipt_path: &str,
    authoritative_report_path: &Path,
) -> Result<CairnLifecycleObservation, String> {
    if native.schema != kiln_core::implementation::NATIVE_GIT_RECEIPT_SCHEMA
        || native.resulting_commit != native.candidate
        || binding.source.kind != WorkSourceKind::CairnTask
        || binding.candidate != native.candidate
        || binding.handoff != native.handoff
        || binding.verification != native.verification
    {
        return Err("native lifecycle receipt does not bind the exact accepted candidate".into());
    }
    observe_lifecycle_at_tree(
        repository,
        AcceptedSource {
            candidate: &native.candidate,
            accepted: &native.resulting_commit,
            tree: &native.tree,
            repository: &native.repository,
        },
        binding,
        receipt_path,
        authoritative_report_path,
    )
}

/// Exact reviewed candidate, accepted commit and tree whose lifecycle receipt is observed.
struct AcceptedSource<'a> {
    candidate: &'a ExternalRef,
    accepted: &'a ExternalRef,
    tree: &'a ExternalRef,
    repository: &'a str,
}

fn observe_lifecycle_at_tree(
    repository: &Path,
    source: AcceptedSource<'_>,
    binding: &CairnLifecycleBinding,
    receipt_path: &str,
    authoritative_report_path: &Path,
) -> Result<CairnLifecycleObservation, String> {
    let AcceptedSource {
        candidate: candidate_ref,
        accepted: accepted_ref,
        tree: expected_tree,
        repository: repository_id,
    } = source;
    let change = binding
        .source
        .native_id
        .split_once('/')
        .map(|(change, _)| change)
        .ok_or("Cairn task has no change identity")?;
    if change.is_empty()
        || !change
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Cairn change identity is malformed".into());
    }
    let repo = Repository::open(repository).map_err(text)?;
    let candidate = repo.find_commit(commit_oid(candidate_ref)?).map_err(text)?;
    let accepted = repo.find_commit(commit_oid(accepted_ref)?).map_err(text)?;
    let candidate_tree = candidate.tree().map_err(text)?;
    let accepted_tree = accepted.tree().map_err(text)?;
    if expected_tree.provider_namespace.as_str() != "git"
        || expected_tree.native_kind.as_str() != "tree"
        || candidate_tree.id().to_string() != expected_tree.native_id.as_str()
        || accepted_tree.id() != candidate_tree.id()
    {
        return Err("accepted lifecycle source is not the reviewed candidate tree".into());
    }
    let receipt_path = relative_path(receipt_path)?;
    if !receipt_path.starts_with(".cairn/") {
        return Err("lifecycle receipt is outside native Cairn source".into());
    }
    let receipt = read_json_blob(&repo, &candidate_tree, receipt_path)?;
    let native = adapt_cairn_sync_archive_receipt(&receipt);
    if !native.valid
        || native.verdict != "PASS"
        || receipt.get("change").and_then(Value::as_str) != Some(change)
        || receipt.get("dry_run").and_then(Value::as_bool) != Some(false)
        || receipt.get("mutated").and_then(Value::as_bool) != Some(true)
        || receipt.get("blocked").and_then(Value::as_bool) != Some(false)
    {
        return Err(
            "native lifecycle receipt is not an applied passing result for this change".into(),
        );
    }
    let mut authoritative = read_json_file(authoritative_report_path)?;
    let expected_hash = authoritative
        .get("receipt_hash")
        .and_then(Value::as_str)
        .ok_or("authoritative report lacks receipt hash")?
        .to_string();
    let authoritative_object = authoritative
        .as_object_mut()
        .ok_or("authoritative report is not an object")?;
    authoritative_object.remove("receipt_hash");
    if expected_hash != binding.authoritative_completion.as_str()
        || canonical_json_hash(&authoritative) != expected_hash
        || authoritative.get("schema").and_then(Value::as_str) != Some("cairn.agent-completion.v2")
        || authoritative.get("valid").and_then(Value::as_bool) != Some(true)
        || authoritative.get("repository_id").and_then(Value::as_str) != Some(repository_id)
        || authoritative.get("policy_hash") != receipt.get("policy_hash")
    {
        return Err(
            "lifecycle policy/report differs from retained authoritative completion".into(),
        );
    }
    let policy = receipt
        .get("policy_hash")
        .and_then(Value::as_str)
        .ok_or("lifecycle policy hash absent")?;
    let lifecycle_policy = ContentId::from_blake3_hex(policy.to_string()).map_err(diagnostic)?;
    let mutation = receipt
        .get("mutation_manifest")
        .ok_or("lifecycle mutation manifest absent")?;
    verify_after_manifest(&repo, &accepted_tree, change, binding.action, mutation)?;
    Ok(CairnLifecycleObservation {
        binding: binding.clone(),
        observed_tree: expected_tree.clone(),
        lifecycle_policy,
        lifecycle_receipt: ExternalRef::new(
            "cairn",
            "lifecycle_receipt",
            &native.normalized_receipt_hash,
        )
        .map_err(diagnostic)?,
    })
}

fn verify_after_manifest(
    repo: &Repository,
    tree: &Tree<'_>,
    change: &str,
    action: CairnLifecycleAction,
    mutation: &Value,
) -> Result<(), String> {
    let kind = match action {
        CairnLifecycleAction::ObserveReviewedSync => "sync",
        CairnLifecycleAction::ObserveReviewedArchive => "archive",
    };
    if mutation.get("kind").and_then(Value::as_str) != Some(kind) {
        return Err("native lifecycle action differs".into());
    }
    let after = mutation
        .get("after")
        .ok_or("native after manifest absent")?;
    let entries = after
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("native after entries absent")?;
    if entries.is_empty() || entries.len() > MAX_MANIFEST_ENTRIES {
        return Err("native lifecycle after manifest is empty or oversized".into());
    }
    if after.get("manifest_hash").and_then(Value::as_str)
        != Some(canonical_json_hash(&serde_json::json!({"entries": entries})).as_str())
    {
        return Err("native lifecycle after manifest hash differs".into());
    }
    let before = mutation
        .get("before")
        .ok_or("native before manifest absent")?;
    if mutation.get("manifest_hash").and_then(Value::as_str)
        != Some(
            canonical_json_hash(&serde_json::json!({"before": before, "after": after})).as_str(),
        )
    {
        return Err("native lifecycle mutation manifest hash differs".into());
    }
    let mut paths = BTreeSet::new();
    let mut positive = 0;
    let mut archived_tasks = false;
    let mut archived_proposal = false;
    let mut archive_root = None;
    let mut observed_bytes = 0usize;
    for entry in entries {
        let path = relative_path(
            entry
                .get("path")
                .and_then(Value::as_str)
                .ok_or("native after path absent")?,
        )?;
        if !paths.insert(path) {
            return Err("duplicate lifecycle after path".into());
        }
        let exists = entry
            .get("exists")
            .and_then(Value::as_bool)
            .ok_or("native after existence unknown")?;
        match action {
            CairnLifecycleAction::ObserveReviewedSync if !path.starts_with(".cairn/specs/") => {
                return Err("sync post-state is outside accepted specifications".into());
            }
            CairnLifecycleAction::ObserveReviewedArchive => {
                if exists {
                    let archive = path
                        .strip_prefix(".cairn/archive/")
                        .and_then(|path| path.split_once('/'))
                        .ok_or("archive lacks positive native archive path")?;
                    if !native_archive_name(archive.0, change) {
                        return Err("archive destination belongs to another change".into());
                    }
                    if archive_root
                        .replace(archive.0)
                        .is_some_and(|previous| previous != archive.0)
                    {
                        return Err("archive post-state spans multiple archive identities".into());
                    }
                    archived_tasks |= archive.1 == "tasks.md";
                    archived_proposal |= archive.1 == "proposal.md";
                } else if path != format!(".cairn/changes/{change}") {
                    return Err("archive absence is not the exact original change directory".into());
                }
            }
            _ => {}
        }
        match tree.get_path(Path::new(path)) {
            Ok(remote) if exists => {
                if remote.kind() != Some(ObjectType::Blob)
                    || !matches!(remote.filemode(), 0o100644 | 0o100755)
                {
                    return Err("lifecycle post-state is not a regular source blob".into());
                }
                let blob = repo.find_blob(remote.id()).map_err(text)?;
                if blob.size() > MAX_SOURCE_BYTES {
                    return Err("lifecycle source blob exceeds bound".into());
                }
                observed_bytes = observed_bytes
                    .checked_add(blob.size())
                    .ok_or("lifecycle source byte count overflow")?;
                if observed_bytes > MAX_OBSERVED_SOURCE_BYTES {
                    return Err("lifecycle source observation exceeds aggregate bound".into());
                }
                let digest = blake3::hash(blob.content()).to_hex();
                if entry.get("content_hash").and_then(Value::as_str) != Some(digest.as_str()) {
                    return Err(
                        "lifecycle accepted source content differs from native receipt".into(),
                    );
                }
                positive += 1;
            }
            Err(error)
                if error.code() == ErrorCode::NotFound
                    && !exists
                    && entry.get("content_hash").is_some_and(Value::is_null) => {}
            _ => {
                return Err(
                    "lifecycle accepted source existence differs from native receipt".into(),
                );
            }
        }
    }
    if positive == 0
        || (matches!(action, CairnLifecycleAction::ObserveReviewedArchive)
            && (!archived_tasks || !archived_proposal))
    {
        return Err(
            "lifecycle lacks positive accepted source/archive identity; absence is insufficient"
                .into(),
        );
    }
    Ok(())
}

fn native_archive_name(name: &str, change: &str) -> bool {
    if name.len() != change.len() + 11 || name.get(11..) != Some(change) {
        return false;
    }
    name.bytes()
        .take(11)
        .enumerate()
        .all(|(index, byte)| match index {
            4 | 7 | 10 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn relative_path(path: &str) -> Result<&str, String> {
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\\')
        || path.contains('\0')
        || !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("native lifecycle path is not a bounded repository-relative path".into());
    }
    Ok(path)
}
fn commit_oid(reference: &ExternalRef) -> Result<Oid, String> {
    if reference.provider_namespace.as_str() != "git" || reference.native_kind.as_str() != "commit"
    {
        return Err("lifecycle source is not a Git commit".into());
    }
    Oid::from_str(reference.native_id.as_str()).map_err(text)
}
fn read_json_blob(repo: &Repository, tree: &Tree<'_>, path: &str) -> Result<Value, String> {
    let entry = tree.get_path(Path::new(path)).map_err(text)?;
    if entry.kind() != Some(ObjectType::Blob) || entry.filemode() != 0o100644 {
        return Err("native receipt is not a regular nonexecutable blob".into());
    }
    let blob = repo.find_blob(entry.id()).map_err(text)?;
    if blob.size() > MAX_RECEIPT_BYTES {
        return Err("native receipt exceeds byte bound".into());
    }
    serde_json::from_slice(blob.content()).map_err(text)
}
fn read_json_file(path: &Path) -> Result<Value, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(text)?
        .take((MAX_RECEIPT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err("authoritative report exceeds byte bound".into());
    }
    serde_json::from_slice(&bytes).map_err(text)
}
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn diagnostic(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

#[cfg(test)]
mod tests;
