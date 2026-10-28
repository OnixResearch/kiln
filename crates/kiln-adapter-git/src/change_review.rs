//! Explicitly writable, local-only collaboration adapter. This module does not
//! alter the authority or behavior of the read-only source adapter.
//! r[impl kiln.review.provider]
//! r[impl kiln.review.parity.ergonomics]
//! r[impl kiln.review.parity.queue]
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use git2::{
    Delta, DiffFindOptions, DiffOptions, Oid, Patch, Repository, RepositoryOpenFlags, Signature,
    Time,
};
use kiln_provider_api::change_review::*;
use serde::{Deserialize, Serialize};

const MAX_FILES: u32 = 1_024;
const MAX_LINES: u32 = 100_000;
const MAX_BYTES: u32 = 8_000_000;
const MAX_REBASE_COMMITS: usize = 256;
const MAX_TARGET_REFS: usize = 64;
const MAX_REF_BYTES: usize = 1_024;
const MAX_IDENTITY_BYTES: usize = 256;
const MAX_PATH_BYTES: usize = 4_096;
const CAPABILITY_COUNT: usize = 9;
const MAX_SUGGESTIONS: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalGitReviewProfile {
    pub repository_id: String,
    pub repository_path: PathBuf,
    pub target_refs: Vec<String>,
    pub capabilities: Vec<ReviewCapability>,
}

#[derive(Debug)]
pub struct LocalGitReviewProvider {
    profile: LocalGitReviewProfile,
}

fn failure(kind: ReviewErrorKind, message: impl Into<String>) -> ReviewError {
    ReviewError {
        kind,
        message: message.into(),
    }
}
fn storage(error: git2::Error) -> ReviewError {
    failure(ReviewErrorKind::Storage, error.message())
}
fn oid(value: &str) -> ReviewResult<Oid> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(failure(
            ReviewErrorKind::InvalidRequest,
            "expected exact lowercase SHA1 commit ID",
        ));
    }
    Oid::from_str(value).map_err(storage)
}
fn commit<'a>(repo: &'a Repository, value: &str) -> ReviewResult<git2::Commit<'a>> {
    repo.find_commit(oid(value)?)
        .map_err(|e| failure(ReviewErrorKind::NotFound, e.message()))
}
fn valid_target(value: &str) -> bool {
    value.len() <= MAX_REF_BYTES
        && value.starts_with("refs/heads/")
        && !value.contains('\0')
        && !value.ends_with('/')
        && git2::Reference::is_valid_name(value)
}
fn target(repo: &Repository, name: &str) -> ReviewResult<Oid> {
    let reference = repo.find_reference(name).map_err(storage)?;
    let id = reference.target().ok_or_else(|| {
        failure(
            ReviewErrorKind::InvalidRequest,
            "symbolic target refs are not supported",
        )
    })?;
    repo.find_commit(id).map_err(storage)?;
    Ok(id)
}
fn signature(value: &ReviewSignature) -> ReviewResult<Signature<'_>> {
    if value.name.is_empty()
        || value.email.is_empty()
        || value.name.len() > MAX_IDENTITY_BYTES
        || value.email.len() > MAX_IDENTITY_BYTES
        || [&value.name, &value.email]
            .iter()
            .any(|s| s.contains(['\0', '\n', '\r', '<', '>']))
    {
        return Err(failure(
            ReviewErrorKind::InvalidRequest,
            "invalid operation signature",
        ));
    }
    Signature::new(&value.name, &value.email, &Time::new(value.time_seconds, 0)).map_err(storage)
}

impl LocalGitReviewProvider {
    pub fn open(mut profile: LocalGitReviewProfile) -> ReviewResult<Self> {
        if profile.target_refs.len() > MAX_TARGET_REFS
            || profile.capabilities.len() > CAPABILITY_COUNT
            || profile.target_refs.iter().any(|r| !valid_target(r))
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "profile target/capability list exceeds provider bounds",
            ));
        }
        let duplicate_targets = profile
            .target_refs
            .iter()
            .enumerate()
            .any(|(index, value)| {
                profile
                    .target_refs
                    .iter()
                    .take(index)
                    .any(|other| other == value)
            });
        let duplicate_capabilities =
            profile
                .capabilities
                .iter()
                .enumerate()
                .any(|(index, value)| {
                    profile
                        .capabilities
                        .iter()
                        .take(index)
                        .any(|other| other == value)
                });
        if profile.repository_id.is_empty()
            || profile.repository_id.len() > MAX_IDENTITY_BYTES
            || profile.repository_id.trim() != profile.repository_id
            || profile.repository_id.chars().any(char::is_control)
            || !profile.repository_path.is_absolute()
            || profile.repository_path.as_os_str().len() > MAX_PATH_BYTES
            || profile.target_refs.is_empty()
            || duplicate_targets
            || duplicate_capabilities
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "bounded unique capabilities, repository identity, absolute path and exact refs/heads targets required",
            ));
        }
        profile.repository_path = profile
            .repository_path
            .canonicalize()
            .map_err(|e| failure(ReviewErrorKind::Storage, e.to_string()))?;
        let adapter = Self { profile };
        let repo = adapter.repository()?;
        for name in &adapter.profile.target_refs {
            target(&repo, name)?;
        }
        Ok(adapter)
    }

    fn repository(&self) -> ReviewResult<Repository> {
        let repo = Repository::open_ext(
            &self.profile.repository_path,
            RepositoryOpenFlags::NO_SEARCH,
            std::iter::empty::<&Path>(),
        )
        .map_err(storage)?;
        let actual = repo
            .workdir()
            .unwrap_or(repo.path())
            .canonicalize()
            .map_err(|e| failure(ReviewErrorKind::Storage, e.to_string()))?;
        if actual != self.profile.repository_path {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "configured repository path is not an exact repository root",
            ));
        }
        if repo.is_shallow() {
            return Err(failure(
                ReviewErrorKind::Unsupported,
                "shallow review repositories are not supported",
            ));
        }
        Ok(repo)
    }

    fn require(&self, capability: ReviewCapability) -> ReviewResult<()> {
        if !self.profile.capabilities.contains(&capability) {
            return Err(failure(
                ReviewErrorKind::Unsupported,
                "review capability is not explicitly enabled",
            ));
        }
        Ok(())
    }
    fn require_target(&self, name: &str) -> ReviewResult<()> {
        if !valid_target(name) || !self.profile.target_refs.iter().any(|r| r == name) {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "target ref is outside the exact allowlist",
            ));
        }
        Ok(())
    }
    fn validate(
        &self,
        repo: &Repository,
        request: &ReviewOperationRequest,
        executing: bool,
    ) -> ReviewResult<()> {
        self.require(if executing {
            request.operation.capability()
        } else {
            ReviewCapability::Inspect
        })?;
        self.require_target(&request.target_ref)?;
        if request.version != REVIEW_PORT_VERSION
            || request.repository_id != self.profile.repository_id
            || request.operation_id.len() != 64
            || !request
                .operation_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "invalid version, repository identity or BLAKE3 operation identity",
            ));
        }
        signature(&request.signature)?;
        target(repo, &request.target_ref)?;
        match &request.operation {
            ReviewOperation::Register { commit: id } => {
                commit(repo, id)?;
            }
            ReviewOperation::Submit {
                commit: id,
                expected_target,
            } => {
                let selected = commit(repo, id)?;
                let expected = commit(repo, expected_target)?;
                if selected.id() == expected.id()
                    || !repo
                        .graph_descendant_of(selected.id(), expected.id())
                        .map_err(storage)?
                {
                    return Err(failure(
                        ReviewErrorKind::Conflict,
                        "submit requires a strict fast-forward of the exact expected target",
                    ));
                }
            }
            ReviewOperation::Rebase { commit: id, onto }
            | ReviewOperation::Revert { commit: id, onto }
            | ReviewOperation::CherryPick { commit: id, onto } => {
                commit(repo, id)?;
                commit(repo, onto)?;
            }
            ReviewOperation::ApplySuggestions {
                commit: id,
                suggestions,
            } => {
                commit(repo, id)?;
                validate_suggestions(suggestions)?;
            }
            ReviewOperation::BuildIntegration {
                commit: id,
                expected_target,
                ..
            } => {
                commit(repo, id)?;
                commit(repo, expected_target)?;
            }
        }
        Ok(())
    }

    fn request_blob(
        repo: &Repository,
        request: &ReviewOperationRequest,
        persist: bool,
    ) -> ReviewResult<Oid> {
        // Length-prefixed UTF-8 fields avoid ambiguous concatenation. Versioning
        // is part of the persisted identity; this is not a provider-native ID.
        let mut text = format!("kiln-review-operation:{}\n", request.version);
        for field in [
            &request.operation_id,
            &request.repository_id,
            &request.target_ref,
            &request.signature.name,
            &request.signature.email,
        ] {
            text.push_str(&format!("{}:{}", field.len(), field));
        }
        text.push_str(&format!("\n{}\n", request.signature.time_seconds));
        let (kind, id, base) = match &request.operation {
            ReviewOperation::Register { commit } => ("register", commit.as_str(), ""),
            ReviewOperation::Submit {
                commit,
                expected_target,
            } => ("submit", commit.as_str(), expected_target.as_str()),
            ReviewOperation::Rebase { commit, onto } => ("rebase", commit.as_str(), onto.as_str()),
            ReviewOperation::Revert { commit, onto } => ("revert", commit.as_str(), onto.as_str()),
            ReviewOperation::CherryPick { commit, onto } => {
                ("cherry_pick", commit.as_str(), onto.as_str())
            }
            ReviewOperation::ApplySuggestions {
                commit,
                suggestions,
            } => {
                text.push_str("suggestions\n");
                text.push_str(&format!("{}\n", suggestions.len()));
                for edit in suggestions {
                    for field in [&edit.path, &edit.expected_original, &edit.replacement] {
                        text.push_str(&format!("{}:{}", field.len(), field));
                    }
                    text.push_str(&format!("\n{}:{}\n", edit.start_line, edit.end_line));
                }
                ("apply_suggestions", commit.as_str(), "")
            }
            ReviewOperation::BuildIntegration {
                commit,
                expected_target,
                strategy,
            } => {
                text.push_str(match strategy {
                    IntegrationStrategy::Merge => "merge\n",
                    IntegrationStrategy::Rebase => "rebase\n",
                });
                (
                    "build_integration",
                    commit.as_str(),
                    expected_target.as_str(),
                )
            }
        };
        text.push_str(&format!("{kind}\n{id}\n{base}\n"));
        if persist {
            repo.blob(text.as_bytes()).map_err(storage)
        } else {
            Oid::hash_object(git2::ObjectType::Blob, text.as_bytes()).map_err(storage)
        }
    }
    fn operation_refs(request: &ReviewOperationRequest) -> (String, String) {
        let root = format!("refs/kiln/review-operations/{}", request.operation_id);
        (format!("{root}/request"), format!("{root}/result"))
    }
    fn check_journal(
        repo: &Repository,
        request: &ReviewOperationRequest,
        create: bool,
    ) -> ReviewResult<bool> {
        let (name, _) = Self::operation_refs(request);
        let expected = Self::request_blob(repo, request, false)?;
        match repo.find_reference(&name) {
            Ok(reference) if reference.target() == Some(expected) => Ok(true),
            Ok(_) => Err(failure(
                ReviewErrorKind::InvalidRequest,
                "operation identity was already used for a different exact request",
            )),
            Err(e) if e.code() == git2::ErrorCode::NotFound && !create => Ok(false),
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                let mut tx = repo.transaction().map_err(storage)?;
                tx.lock_ref(&name).map_err(storage)?;
                if Self::check_journal(repo, request, false)? {
                    return Ok(true);
                }
                Self::request_blob(repo, request, true)?;
                let sig = signature(&request.signature)?;
                tx.set_target(
                    &name,
                    expected,
                    Some(&sig),
                    "prepare exact review operation",
                )
                .map_err(storage)?;
                tx.commit().map_err(storage)?;
                Ok(true)
            }
            Err(e) => Err(storage(e)),
        }
    }
    fn observed(
        &self,
        repo: &Repository,
        request: &ReviewOperationRequest,
    ) -> ReviewResult<ReviewEffect> {
        let current = target(repo, &request.target_ref)?;
        let prepared = Self::check_journal(repo, request, false)?;
        let (_, result_ref) = Self::operation_refs(request);
        match repo.find_reference(&result_ref) {
            Ok(result) => {
                if !prepared {
                    return Ok(ReviewEffect::Uncertain {
                        message: "result exists without exact request journal".into(),
                    });
                }
                let id = result.target().ok_or_else(|| {
                    failure(ReviewErrorKind::Uncertain, "invalid operation result ref")
                })?;
                repo.find_commit(id).map_err(storage)?;
                return Ok(ReviewEffect::Applied {
                    commit: id.to_string(),
                    target_commit: current.to_string(),
                });
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => {}
            Err(e) => return Err(storage(e)),
        }
        if let ReviewOperation::Submit {
            commit,
            expected_target,
        } = &request.operation
        {
            if prepared && current == oid(commit)? {
                return Ok(ReviewEffect::Applied {
                    commit: commit.clone(),
                    target_commit: current.to_string(),
                });
            }
            let message = format!("kiln-review:{}", request.operation_id);
            let possibly_written = if prepared {
                let expected = oid(expected_target)?;
                let proposed = oid(commit)?;
                match repo.reflog(&request.target_ref) {
                    Ok(log) => log.iter().any(|entry| {
                        entry.message() == Some(message.as_str())
                            && entry.id_old() == expected
                            && entry.id_new() == proposed
                    }),
                    Err(e) if e.code() == git2::ErrorCode::NotFound => false,
                    Err(e) => return Err(storage(e)),
                }
            } else {
                false
            };
            // A reflog can be written before its ref update. It proves an
            // attempted effect, not success, after another writer moved target.
            if possibly_written || (prepared && current != oid(expected_target)?) {
                return Ok(ReviewEffect::Uncertain {
                    message:
                        "target moved or an interrupted submit requires exact effect inspection"
                            .into(),
                });
            }
        }
        Ok(ReviewEffect::NotApplied {
            target_commit: current.to_string(),
        })
    }

    fn derived_commit(
        &self,
        repo: &Repository,
        request: &ReviewOperationRequest,
    ) -> ReviewResult<Oid> {
        let sig = signature(&request.signature)?;
        let (id, onto, kind) = match &request.operation {
            ReviewOperation::Register { commit } | ReviewOperation::Submit { commit, .. } => {
                return oid(commit);
            }
            ReviewOperation::ApplySuggestions {
                commit: id,
                suggestions,
            } => {
                return suggestion_commit(repo, request, id, suggestions);
            }
            ReviewOperation::BuildIntegration {
                commit: id,
                expected_target,
                strategy,
            } => {
                if *strategy == IntegrationStrategy::Merge {
                    let source = commit(repo, id)?;
                    let parent = commit(repo, expected_target)?;
                    let mut index = repo
                        .merge_commits(&parent, &source, None)
                        .map_err(|e| failure(ReviewErrorKind::Conflict, e.message()))?;
                    if index.has_conflicts() {
                        return Err(failure(
                            ReviewErrorKind::Conflict,
                            "exact integration has conflicts",
                        ));
                    }
                    let tree_id = index.write_tree_to(repo).map_err(storage)?;
                    let tree = repo.find_tree(tree_id).map_err(storage)?;
                    let message = format!(
                        "Kiln integration\n\nKiln-Source: {id}\nKiln-Target: {expected_target}\nKiln-Operation: {}\n",
                        request.operation_id
                    );
                    return repo
                        .commit(None, &sig, &sig, &message, &tree, &[&parent, &source])
                        .map_err(storage);
                }
                (id, expected_target, "rebase")
            }
            ReviewOperation::Rebase { commit, onto } => (commit, onto, "rebase"),
            ReviewOperation::Revert { commit, onto } => (commit, onto, "revert"),
            ReviewOperation::CherryPick { commit, onto } => (commit, onto, "cherry-pick"),
        };
        let source = commit(repo, id)?;
        let mut parent = commit(repo, onto)?;
        let mut replay = Vec::new();
        if kind == "rebase" {
            let base = repo
                .merge_base(source.id(), parent.id())
                .map_err(|e| failure(ReviewErrorKind::Conflict, e.message()))?;
            let mut cursor = source;
            while cursor.id() != base {
                if cursor.parent_count() != 1 || replay.len() >= MAX_REBASE_COMMITS {
                    return Err(failure(
                        ReviewErrorKind::Unsupported,
                        "rebase requires at most 256 linear commits from the merge base",
                    ));
                }
                let next = cursor.parent(0).map_err(storage)?;
                replay.push(cursor);
                cursor = next;
            }
            replay.reverse();
        } else {
            if source.parent_count() != 1 {
                return Err(failure(
                    ReviewErrorKind::Unsupported,
                    "revert/cherry-pick requires one unambiguous parent",
                ));
            }
            replay.push(source);
        }
        for original in replay {
            let mut index = if kind == "revert" {
                repo.revert_commit(&original, &parent, 0, None)
            } else {
                repo.cherrypick_commit(&original, &parent, 0, None)
            }
            .map_err(|e| failure(ReviewErrorKind::Conflict, e.message()))?;
            if index.has_conflicts() {
                return Err(failure(
                    ReviewErrorKind::Conflict,
                    "exact revision transformation has conflicts",
                ));
            }
            let tree_id = index.write_tree_to(repo).map_err(storage)?;
            let tree = repo.find_tree(tree_id).map_err(storage)?;
            let message = std::str::from_utf8(original.message_bytes()).map_err(|_| {
                failure(
                    ReviewErrorKind::Unsupported,
                    "non-UTF8 commit messages are not supported for author transformations",
                )
            })?;
            let message = format!(
                "{message}\n\nKiln-{kind}: {}\nKiln-Operation: {}\n",
                original.id(),
                request.operation_id
            );
            let author = if kind == "revert" {
                sig.to_owned()
            } else {
                original.author().to_owned()
            };
            let result = repo
                .commit(None, &author, &sig, &message, &tree, &[&parent])
                .map_err(storage)?;
            parent = repo.find_commit(result).map_err(storage)?;
        }
        Ok(parent.id())
    }
}

impl ChangeReviewPort for LocalGitReviewProvider {
    fn info(&self) -> ReviewProviderInfo {
        ReviewProviderInfo {
            version: REVIEW_PORT_VERSION,
            repository_id: self.profile.repository_id.clone(),
            capabilities: self.profile.capabilities.clone(),
            target_refs: self.profile.target_refs.clone(),
        }
    }
    fn inspect(&self, id: &str, target_ref: &str) -> ReviewResult<CommitInspection> {
        self.require(ReviewCapability::Inspect)?;
        self.require_target(target_ref)?;
        let repo = self.repository()?;
        let selected = commit(&repo, id)?;
        let current = target(&repo, target_ref)?;
        Ok(CommitInspection {
            repository_id: self.profile.repository_id.clone(),
            commit: selected.id().to_string(),
            parents: selected.parent_ids().map(|p| p.to_string()).collect(),
            target_ref: target_ref.into(),
            target_commit: current.to_string(),
            ancestor_of_target: selected.id() == current
                || repo
                    .graph_descendant_of(current, selected.id())
                    .map_err(storage)?,
            target_is_ancestor: selected.id() == current
                || repo
                    .graph_descendant_of(selected.id(), current)
                    .map_err(storage)?,
        })
    }
    fn compare(&self, request: &CompareRequest) -> ReviewResult<CommitComparison> {
        self.require(ReviewCapability::Compare)?;
        let limits = request.limits;
        if limits.max_files == 0
            || limits.max_files > MAX_FILES
            || limits.max_lines == 0
            || limits.max_lines > MAX_LINES
            || limits.max_bytes == 0
            || limits.max_bytes > MAX_BYTES
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "diff bounds must be positive and within provider limits",
            ));
        }
        let repo = self.repository()?;
        let old_tree = commit(&repo, &request.old_commit)?
            .tree()
            .map_err(storage)?;
        let new_tree = commit(&repo, &request.new_commit)?
            .tree()
            .map_err(storage)?;
        let mut options = DiffOptions::new();
        options
            .context_lines(3)
            .max_size(i64::from(limits.max_bytes));
        match request.whitespace {
            WhitespaceMode::Exact => {}
            WhitespaceMode::IgnoreAll => {
                options.ignore_whitespace(true);
            }
            WhitespaceMode::IgnoreChange => {
                options.ignore_whitespace_change(true);
            }
            WhitespaceMode::IgnoreEol => {
                options.ignore_whitespace_eol(true);
            }
        }
        let mut diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut options))
            .map_err(storage)?;
        let mut find = DiffFindOptions::new();
        find.renames(true).rename_limit(MAX_FILES as usize);
        diff.find_similar(Some(&mut find)).map_err(storage)?;
        let mut result = CommitComparison {
            repository_id: self.profile.repository_id.clone(),
            old_commit: request.old_commit.clone(),
            new_commit: request.new_commit.clone(),
            files: Vec::new(),
            truncated: false,
        };
        let mut remaining_lines = limits.max_lines;
        let mut remaining_bytes = limits.max_bytes as usize;
        for (position, delta) in diff.deltas().enumerate() {
            if result.files.len() >= limits.max_files as usize {
                result.truncated = true;
                break;
            }
            let old = delta.old_file();
            let new = delta.new_file();
            let old_exists = delta.status() != Delta::Added;
            let new_exists = delta.status() != Delta::Deleted;
            let mut file = DiffFile {
                old_path: if old_exists {
                    exact_path(old.path_bytes())?
                } else {
                    None
                },
                new_path: if new_exists {
                    exact_path(new.path_bytes())?
                } else {
                    None
                },
                old_blob: if old_exists {
                    Some(old.id().to_string())
                } else {
                    None
                },
                new_blob: if new_exists {
                    Some(new.id().to_string())
                } else {
                    None
                },
                old_line_count: None,
                new_line_count: None,
                renamed: delta.status() == Delta::Renamed,
                binary: false,
                truncated: false,
                hunks: Vec::new(),
            };
            let old_facts = if old.mode() == git2::FileMode::Commit {
                (None, true, false)
            } else {
                blob_facts(&repo, old.id(), limits.max_bytes)?
            };
            let new_facts = if new.mode() == git2::FileMode::Commit {
                (None, true, false)
            } else {
                blob_facts(&repo, new.id(), limits.max_bytes)?
            };
            file.old_line_count = old_facts.0;
            file.new_line_count = new_facts.0;
            file.binary = old_facts.1 || new_facts.1;
            if old_facts.2 || new_facts.2 || remaining_lines == 0 || remaining_bytes == 0 {
                file.truncated = true;
            } else if !file.binary {
                if let Some(patch) = Patch::from_diff(&diff, position).map_err(storage)? {
                    for h in 0..patch.num_hunks() {
                        let (header, count) = patch.hunk(h).map_err(storage)?;
                        let header_bytes = header.header();
                        if header_bytes.len() > remaining_bytes {
                            file.truncated = true;
                            break;
                        }
                        remaining_bytes -= header_bytes.len();
                        let mut hunk = DiffHunk {
                            old_start: header.old_start(),
                            old_lines: header.old_lines(),
                            new_start: header.new_start(),
                            new_lines: header.new_lines(),
                            header: String::from_utf8_lossy(header_bytes).into_owned(),
                            lines: Vec::new(),
                        };
                        for l in 0..count {
                            let line = patch.line_in_hunk(h, l).map_err(storage)?;
                            if remaining_lines == 0 || line.content().len() > remaining_bytes {
                                file.truncated = true;
                                break;
                            }
                            remaining_lines -= 1;
                            remaining_bytes -= line.content().len();
                            let kind = match line.origin() {
                                '+' => DiffLineKind::Addition,
                                '-' => DiffLineKind::Deletion,
                                ' ' => DiffLineKind::Context,
                                _ => DiffLineKind::NoNewline,
                            };
                            hunk.lines.push(DiffLine {
                                kind,
                                old_line: line.old_lineno(),
                                new_line: line.new_lineno(),
                                text: String::from_utf8_lossy(line.content()).into_owned(),
                            });
                        }
                        file.hunks.push(hunk);
                        if file.truncated {
                            break;
                        }
                    }
                }
            }
            result.truncated |= file.truncated;
            result.files.push(file);
        }
        Ok(result)
    }
    fn execute(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        let repo = self.repository()?;
        self.validate(&repo, request, true)?;
        match self.observed(&repo, request)? {
            applied @ ReviewEffect::Applied { .. } => return Ok(applied),
            ReviewEffect::Uncertain { message } => {
                return Err(failure(ReviewErrorKind::Uncertain, message));
            }
            ReviewEffect::NotApplied { target_commit } => {
                if let ReviewOperation::Submit {
                    expected_target, ..
                }
                | ReviewOperation::BuildIntegration {
                    expected_target, ..
                } = &request.operation
                {
                    if target_commit != *expected_target {
                        return Err(failure(
                            ReviewErrorKind::TargetMoved,
                            "exact target changed before submit",
                        ));
                    }
                }
            }
        }
        Self::check_journal(&repo, request, true)?;
        let (_, result_ref) = Self::operation_refs(request);
        // Holding this ref lock serializes retries of the same operation across
        // processes, including the target-mutation/result-journal crash window.
        let mut result_tx = repo.transaction().map_err(storage)?;
        result_tx.lock_ref(&result_ref).map_err(storage)?;
        match self.observed(&repo, request)? {
            applied @ ReviewEffect::Applied { .. } => return Ok(applied),
            ReviewEffect::Uncertain { message } => {
                return Err(failure(ReviewErrorKind::Uncertain, message));
            }
            ReviewEffect::NotApplied { .. } => {}
        }
        let result = self.derived_commit(&repo, request)?;
        if let ReviewOperation::BuildIntegration {
            expected_target, ..
        } = &request.operation
        {
            if target(&repo, &request.target_ref)? != oid(expected_target)? {
                return Err(failure(
                    ReviewErrorKind::TargetMoved,
                    "exact target changed while constructing integration",
                ));
            }
        }
        let sig = signature(&request.signature)?;
        if let ReviewOperation::Submit {
            expected_target, ..
        } = &request.operation
        {
            let expected = oid(expected_target)?;
            // Fast-forward legality was checked before persisting preparation.
            repo.reference_ensure_log(&request.target_ref)
                .map_err(storage)?;
            let mut target_tx = repo.transaction().map_err(storage)?;
            target_tx.lock_ref(&request.target_ref).map_err(storage)?;
            if target(&repo, &request.target_ref)? != expected {
                return Err(failure(
                    ReviewErrorKind::TargetMoved,
                    "exact target changed before submit",
                ));
            }
            target_tx
                .set_target(
                    &request.target_ref,
                    result,
                    Some(&sig),
                    &format!("kiln-review:{}", request.operation_id),
                )
                .map_err(storage)?;
            target_tx
                .commit()
                .map_err(|e| failure(ReviewErrorKind::Uncertain, e.message()))?;
            if target(&repo, &request.target_ref)? != result {
                return Err(failure(
                    ReviewErrorKind::Uncertain,
                    "target moved after submit; reconcile before retry",
                ));
            }
        }
        result_tx
            .set_target(
                &result_ref,
                result,
                Some(&sig),
                "record exact review operation result",
            )
            .map_err(storage)?;
        result_tx
            .commit()
            .map_err(|e| failure(ReviewErrorKind::Uncertain, e.message()))?;
        Ok(ReviewEffect::Applied {
            commit: result.to_string(),
            target_commit: target(&repo, &request.target_ref)?.to_string(),
        })
    }
    fn reconcile(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        let repo = self.repository()?;
        self.validate(&repo, request, false)?;
        self.observed(&repo, request)
    }
}

fn validate_suggestions(suggestions: &[SuggestionEdit]) -> ReviewResult<()> {
    if suggestions.is_empty() || suggestions.len() > MAX_SUGGESTIONS {
        return Err(failure(
            ReviewErrorKind::InvalidRequest,
            "suggestion batch exceeds bounds",
        ));
    }
    let mut bytes = 0usize;
    for edit in suggestions {
        let path = &edit.path;
        if path.is_empty()
            || path.len() > MAX_PATH_BYTES
            || path.contains(['\\', '\0'])
            || path.chars().any(char::is_control)
            || path.split('/').any(|part| {
                part.is_empty() || part == "." || part == ".." || part.eq_ignore_ascii_case(".git")
            })
            || edit.start_line == 0
            || edit.end_line < edit.start_line
            || edit.expected_original.contains('\0')
            || edit.replacement.contains('\0')
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "invalid exact suggestion path, range or text",
            ));
        }
        bytes = bytes
            .saturating_add(path.len())
            .saturating_add(edit.expected_original.len())
            .saturating_add(edit.replacement.len());
        if bytes > MAX_BYTES as usize {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "suggestion text exceeds bounds",
            ));
        }
    }
    Ok(())
}

fn suggestion_commit(
    repo: &Repository,
    request: &ReviewOperationRequest,
    id: &str,
    suggestions: &[SuggestionEdit],
) -> ReviewResult<Oid> {
    validate_suggestions(suggestions)?;
    let source = commit(repo, id)?;
    let tree = source.tree().map_err(storage)?;
    let mut files: BTreeMap<&str, Vec<&SuggestionEdit>> = BTreeMap::new();
    for edit in suggestions {
        files.entry(&edit.path).or_default().push(edit);
    }
    // Validate every original and overlap before writing even a replacement blob.
    let mut replacements = Vec::with_capacity(files.len());
    let mut total_bytes = 0usize;
    for (path, mut edits) in files {
        edits.sort_by_key(|edit| edit.start_line);
        if edits
            .iter()
            .zip(edits.iter().skip(1))
            .any(|(left, right)| left.end_line >= right.start_line)
        {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "overlapping suggestion ranges",
            ));
        }
        let entry = tree.get_path(Path::new(path)).map_err(|_| {
            failure(
                ReviewErrorKind::NotFound,
                "suggestion file is absent from exact revision",
            )
        })?;
        if !matches!(entry.filemode(), 0o100644 | 0o100755) {
            return Err(failure(
                ReviewErrorKind::Unsupported,
                "suggestions require a regular text blob",
            ));
        }
        let blob = repo.find_blob(entry.id()).map_err(storage)?;
        if blob.size() > MAX_BYTES as usize || blob.is_binary() {
            return Err(failure(
                ReviewErrorKind::Unsupported,
                "binary or oversized suggestion file",
            ));
        }
        let original = std::str::from_utf8(blob.content())
            .map_err(|_| failure(ReviewErrorKind::Unsupported, "suggestion file is not UTF-8"))?;
        let mut lines = original.split_inclusive('\n');
        let mut line_number = 1;
        let mut position = 0;
        let mut updated = String::new();
        let mut cursor = 0;
        for edit in edits {
            while line_number < edit.start_line {
                let line = lines.next().ok_or_else(|| {
                    failure(
                        ReviewErrorKind::InvalidRequest,
                        "suggestion range is outside exact file",
                    )
                })?;
                position += line.len();
                line_number += 1;
            }
            let start = position;
            while line_number <= edit.end_line {
                let line = lines.next().ok_or_else(|| {
                    failure(
                        ReviewErrorKind::InvalidRequest,
                        "suggestion range is outside exact file",
                    )
                })?;
                position += line.len();
                line_number += 1;
            }
            let end = position;
            if original[start..end] != edit.expected_original {
                return Err(failure(
                    ReviewErrorKind::Conflict,
                    "suggestion original no longer matches exact revision",
                ));
            }
            updated.push_str(&original[cursor..start]);
            updated.push_str(&edit.replacement);
            cursor = end;
        }
        updated.push_str(&original[cursor..]);
        total_bytes = total_bytes.saturating_add(updated.len());
        if total_bytes > MAX_BYTES as usize {
            return Err(failure(
                ReviewErrorKind::InvalidRequest,
                "suggestion output exceeds bounds",
            ));
        }
        replacements.push((path, updated));
    }
    let mut index = git2::Index::new().map_err(storage)?;
    index.read_tree(&tree).map_err(storage)?;
    for (path, text) in replacements {
        let mut entry = index
            .get_path(Path::new(path), 0)
            .ok_or_else(|| failure(ReviewErrorKind::NotFound, "suggestion index entry absent"))?;
        entry.id = repo.blob(text.as_bytes()).map_err(storage)?;
        entry.file_size = u32::try_from(text.len()).map_err(|_| {
            failure(
                ReviewErrorKind::InvalidRequest,
                "suggestion output exceeds bounds",
            )
        })?;
        index.add(&entry).map_err(storage)?;
    }
    let tree_id = index.write_tree_to(repo).map_err(storage)?;
    let updated = repo.find_tree(tree_id).map_err(storage)?;
    let sig = signature(&request.signature)?;
    let message = format!(
        "Apply review suggestions\n\nKiln-Source: {id}\nKiln-Operation: {}\n",
        request.operation_id
    );
    repo.commit(None, &sig, &sig, &message, &updated, &[&source])
        .map_err(storage)
}

fn exact_path(bytes: Option<&[u8]>) -> ReviewResult<Option<String>> {
    bytes
        .map(|bytes| {
            if bytes.len() > MAX_PATH_BYTES {
                return Err(failure(
                    ReviewErrorKind::Unsupported,
                    "path exceeds exact review anchor bounds",
                ));
            }
            std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
                failure(
                    ReviewErrorKind::Unsupported,
                    "non-UTF8 paths cannot be represented as exact review anchors",
                )
            })
        })
        .transpose()
}
fn blob_facts(
    repo: &Repository,
    id: Oid,
    max_bytes: u32,
) -> ReviewResult<(Option<u32>, bool, bool)> {
    if id.is_zero() {
        return Ok((None, false, false));
    }
    let object = repo.find_object(id, None).map_err(storage)?;
    match object.into_blob() {
        Ok(blob) => {
            if blob.size() > max_bytes as usize {
                return Ok((None, blob.is_binary(), true));
            }
            let bytes = blob.content();
            let binary = blob.is_binary() || std::str::from_utf8(bytes).is_err();
            let count = if binary {
                None
            } else {
                let lines = bytes.iter().filter(|b| **b == b'\n').count()
                    + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"));
                Some(u32::try_from(lines).map_err(|_| {
                    failure(
                        ReviewErrorKind::InvalidRequest,
                        "file exceeds line coordinate range",
                    )
                })?)
            };
            Ok((count, binary, false))
        }
        Err(_) => Ok((None, true, false)),
    }
}
