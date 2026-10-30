//! Read-only Jujutsu observation shell.
//!
//! Kiln never mutates jj state. The operation head is read from the
//! operation-head store on disk (the one jj code path that could merge heads is
//! never invoked), and every jj process runs with `--ignore-working-copy`
//! (no snapshot) and `--at-op` pinned to that exact head (no operation merge),
//! a cleared environment, fixed JSON templates and named bounds. Revsets are
//! built only from full, validated change IDs and exact commit IDs.
// r[impl kiln.review.jj_intake.read_only]
// r[impl kiln.review.jj_intake.read_only.scenario.unchanged]
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bounded_exec::{
    CommandSpec, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use kiln_core::source::{
    JujutsuBackend, JujutsuFacts, admit_jujutsu_facts, valid_jujutsu_change_id,
};
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef};
use serde::Deserialize;

/// Largest stack (and therefore change-ID list) one observation admits.
pub const MAX_CHANGE_IDS: usize = 32;
/// Full jj operation IDs are 64-byte BLAKE2b digests in lowercase hex.
pub const OPERATION_ID_HEX: usize = 128;
/// Full Git commit IDs for the Git backend.
pub const COMMIT_ID_HEX: usize = 40;
/// Predecessor commits retained per evolog entry.
pub const MAX_PREDECESSORS: usize = 32;
/// Operation-head entries read before refusing the head store as unbounded.
const MAX_OPERATION_HEADS_SCAN: usize = 64;
/// Bytes read from the `.jj/repo` pointer, `store/type` and `store/git_target`.
const MAX_POINTER_BYTES: u64 = 4096;
const JJ_TIMEOUT_MS: u64 = 15_000;
const JJ_POLL_MS: u64 = 25;
const JJ_TEARDOWN_MS: u64 = 2_000;
const JJ_STDOUT_BYTES: usize = 1_048_576;
const JJ_STDERR_BYTES: usize = 8_192;
/// jj loads no user configuration; repository configuration is still read.
const JJ_CONFIG_NONE: &str = "/dev/null";

const LOG_TEMPLATE: &str = r#""[" ++ json(commit_id) ++ "," ++ json(change_id) ++ "," ++ json(divergent) ++ "," ++ json(conflict) ++ "," ++ json(local_bookmarks.filter(|b| b.conflict()).map(|b| b.name())) ++ "]\n""#;
const EVOLOG_TEMPLATE: &str = r#""[" ++ json(commit.commit_id()) ++ "," ++ json(predecessors.map(|c| c.commit_id())) ++ "," ++ if(operation, json(operation.id()), "null") ++ "]\n""#;

/// Configured, operator-admitted jj workspace and executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workspace {
    pub executable: PathBuf,
    pub workspace: PathBuf,
}

/// Exact repository facts read from disk without invoking jj.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryObservation {
    pub backend: JujutsuBackend,
    /// The single operation head; `None` when the head store is empty.
    pub operation: Option<String>,
    /// Number of operation heads observed (more than one is divergent).
    pub operation_heads: usize,
    /// Canonical Git store backing the jj repository, when the backend is Git.
    pub git_store: Option<PathBuf>,
}

/// One visible commit returned for a requested change ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitObservation {
    pub commit: String,
    pub change_id: String,
    pub divergent: bool,
    pub conflict: bool,
    pub conflicted_bookmarks: Vec<String>,
}

/// The newest evolog entry of one exact commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvologObservation {
    pub commit: String,
    pub predecessors: Vec<String>,
    pub operation: Option<String>,
}

/// One change ID admitted through [`JujutsuFacts`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedChange {
    pub change_id: String,
    pub commit: String,
    pub facts: JujutsuFacts,
    pub evolog: EvologObservation,
}

/// Exact observation of a requested ordered change-ID list at one operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub operation: String,
    pub git_store: PathBuf,
    pub changes: Vec<AdmittedChange>,
}

fn diagnostic(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn read_small(path: &Path) -> CoreResult<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| {
        diagnostic(
            "workspace_invalid",
            "jujutsu.workspace",
            "Jujutsu repository metadata is unavailable",
        )
    })?;
    let mut text = String::new();
    file.take(MAX_POINTER_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| {
            diagnostic(
                "workspace_invalid",
                "jujutsu.workspace",
                "Jujutsu repository metadata is not text",
            )
        })?;
    if text.len() as u64 > MAX_POINTER_BYTES {
        return Err(diagnostic(
            "workspace_invalid",
            "jujutsu.workspace",
            "Jujutsu repository metadata exceeds its bound",
        ));
    }
    Ok(text)
}

fn repository_directory(workspace: &Path) -> CoreResult<PathBuf> {
    let dot = workspace.join(".jj");
    let repo = dot.join("repo");
    let metadata = std::fs::symlink_metadata(&repo).map_err(|_| {
        diagnostic(
            "workspace_invalid",
            "jujutsu.workspace",
            "path is not a Jujutsu workspace",
        )
    })?;
    if metadata.is_dir() {
        return Ok(repo);
    }
    if metadata.is_file() {
        // Secondary workspaces store a path to the shared repository.
        let pointer = read_small(&repo)?;
        let target = PathBuf::from(pointer.trim_end_matches('\n'));
        return Ok(if target.is_absolute() {
            target
        } else {
            dot.join(target)
        });
    }
    Err(diagnostic(
        "workspace_invalid",
        "jujutsu.workspace",
        "Jujutsu repository pointer is not a regular file or directory",
    ))
}

/// Read backend, Git store and operation heads from disk. Never invokes jj.
pub fn observe_repository(workspace: &Path) -> CoreResult<RepositoryObservation> {
    let repo = repository_directory(workspace)?;
    let store = repo.join("store");
    let backend = if read_small(&store.join("type"))?.trim_end_matches('\n') == "git" {
        JujutsuBackend::Git
    } else {
        JujutsuBackend::Other
    };
    let git_store = if backend == JujutsuBackend::Git {
        let target = read_small(&store.join("git_target"))?;
        let target = PathBuf::from(target.trim_end_matches('\n'));
        let path = if target.is_absolute() {
            target
        } else {
            store.join(target)
        };
        Some(path.canonicalize().map_err(|_| {
            diagnostic(
                "store_unavailable",
                "jujutsu.git_target",
                "Jujutsu Git store is unavailable",
            )
        })?)
    } else {
        None
    };
    let heads = std::fs::read_dir(repo.join("op_heads").join("heads")).map_err(|_| {
        diagnostic(
            "operation_missing",
            "jujutsu.operation",
            "Jujutsu operation-head store is unavailable",
        )
    })?;
    let mut names = Vec::new();
    for (index, entry) in heads.enumerate() {
        if index >= MAX_OPERATION_HEADS_SCAN {
            return Err(diagnostic(
                "operation_divergent",
                "jujutsu.operation",
                "Jujutsu operation-head store exceeds its bound",
            ));
        }
        let entry = entry.map_err(|_| {
            diagnostic(
                "operation_missing",
                "jujutsu.operation",
                "Jujutsu operation head is unreadable",
            )
        })?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    admit_operation_heads(backend, git_store, &names)
}

/// Pure: exactly one well-formed operation head is admitted.
pub fn admit_operation_heads(
    backend: JujutsuBackend,
    git_store: Option<PathBuf>,
    names: &[String],
) -> CoreResult<RepositoryObservation> {
    if names.iter().any(|name| !lower_hex(name, OPERATION_ID_HEX)) {
        return Err(diagnostic(
            "operation_missing",
            "jujutsu.operation",
            "Jujutsu operation head is malformed",
        ));
    }
    Ok(RepositoryObservation {
        backend,
        operation: match names {
            [single] => Some(single.clone()),
            _ => None,
        },
        operation_heads: names.len(),
        git_store,
    })
}

/// Pure: the jj argument vector shared by every read.
fn base_args(workspace: &Path, operation: &str, command: &str) -> Vec<OsString> {
    vec![
        "--repository".into(),
        workspace.as_os_str().to_owned(),
        "--ignore-working-copy".into(),
        "--at-op".into(),
        operation.into(),
        "--no-pager".into(),
        "--color".into(),
        "never".into(),
        command.into(),
        "--no-graph".into(),
    ]
}

/// Pure: `change_id(a) | change_id(b) | ...` over full validated IDs only.
pub fn change_revset(change_ids: &[String]) -> CoreResult<String> {
    if change_ids.is_empty() || change_ids.len() > MAX_CHANGE_IDS {
        return Err(diagnostic(
            "stack_bound",
            "jujutsu.change_ids",
            "Jujutsu change-ID list is empty or exceeds its bound",
        ));
    }
    let mut revset = String::new();
    for (index, id) in change_ids.iter().enumerate() {
        if !valid_jujutsu_change_id(id) {
            return Err(diagnostic(
                "change_id_invalid",
                "jujutsu.change_id",
                "only full exact Jujutsu change IDs are admitted",
            ));
        }
        if change_ids.iter().take(index).any(|other| other == id) {
            return Err(diagnostic(
                "change_duplicate",
                "jujutsu.change_ids",
                "a Jujutsu change ID appears twice",
            ));
        }
        if index > 0 {
            revset.push_str(" | ");
        }
        revset.push_str("change_id(");
        revset.push_str(id);
        revset.push(')');
    }
    Ok(revset)
}

/// Pure: the `jj log` arguments for an exact change-ID list.
pub fn log_args(
    workspace: &Path,
    operation: &str,
    change_ids: &[String],
) -> CoreResult<Vec<OsString>> {
    let mut args = base_args(workspace, operation, "log");
    args.extend([
        "-r".into(),
        change_revset(change_ids)?.into(),
        "-T".into(),
        LOG_TEMPLATE.into(),
    ]);
    Ok(args)
}

/// Pure: the `jj evolog` arguments for one exact commit.
pub fn evolog_args(workspace: &Path, operation: &str, commit: &str) -> CoreResult<Vec<OsString>> {
    if !lower_hex(commit, COMMIT_ID_HEX) {
        return Err(observation_invalid());
    }
    let mut args = base_args(workspace, operation, "evolog");
    args.extend([
        "-r".into(),
        commit.into(),
        "--limit".into(),
        "1".into(),
        "-T".into(),
        EVOLOG_TEMPLATE.into(),
    ]);
    Ok(args)
}

fn observation_invalid() -> Diagnostic {
    diagnostic(
        "observation_invalid",
        "jujutsu.output",
        "Jujutsu output did not match the fixed template",
    )
}

#[derive(Deserialize)]
struct LogLine(String, String, bool, bool, Vec<String>);

#[derive(Deserialize)]
struct EvologLine(String, Vec<String>, Option<String>);

/// Pure: parse the fixed `jj log` template output, one JSON array per line.
pub fn parse_log(stdout: &[u8]) -> CoreResult<Vec<CommitObservation>> {
    let text = std::str::from_utf8(stdout).map_err(|_| observation_invalid())?;
    let mut result = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let LogLine(commit, change_id, divergent, conflict, conflicted_bookmarks) =
            serde_json::from_str(line).map_err(|_| observation_invalid())?;
        if !lower_hex(&commit, COMMIT_ID_HEX)
            || !valid_jujutsu_change_id(&change_id)
            || result.len() >= MAX_CHANGE_IDS * MAX_PREDECESSORS
        {
            return Err(observation_invalid());
        }
        result.push(CommitObservation {
            commit,
            change_id,
            divergent,
            conflict,
            conflicted_bookmarks,
        });
    }
    Ok(result)
}

/// Pure: parse the fixed `jj evolog --limit 1` template output.
pub fn parse_evolog(stdout: &[u8], commit: &str) -> CoreResult<EvologObservation> {
    let text = std::str::from_utf8(stdout).map_err(|_| observation_invalid())?;
    let mut lines = text.lines().filter(|line| !line.is_empty());
    let (Some(line), None) = (lines.next(), lines.next()) else {
        return Err(observation_invalid());
    };
    let EvologLine(observed, predecessors, operation) =
        serde_json::from_str(line).map_err(|_| observation_invalid())?;
    if observed != commit
        || predecessors.len() > MAX_PREDECESSORS
        || predecessors
            .iter()
            .any(|predecessor| !lower_hex(predecessor, COMMIT_ID_HEX))
        || operation
            .as_deref()
            .is_some_and(|operation| !lower_hex(operation, OPERATION_ID_HEX))
    {
        return Err(observation_invalid());
    }
    Ok(EvologObservation {
        commit: observed,
        predecessors,
        operation,
    })
}

/// Pure: exactly one operation head is the operation identity.
pub fn admitted_operation(repository: &RepositoryObservation) -> CoreResult<&str> {
    match (&repository.operation, repository.operation_heads) {
        (Some(operation), 1) => Ok(operation),
        (_, 0) => Err(diagnostic(
            "operation_missing",
            "jujutsu.operation",
            "Jujutsu repository observation has no operation identity",
        )),
        _ => Err(diagnostic(
            "operation_divergent",
            "jujutsu.operation",
            "Jujutsu repository has more than one operation head",
        )),
    }
}

/// Pure: build and admit the jj source facts for one requested change ID.
// r[impl kiln.review.jj_intake.admission]
// r[impl kiln.review.jj_intake.admission.scenario.divergent]
// r[impl kiln.review.jj_intake.admission.scenario.conflicted]
pub fn admit_change(
    repository: &RepositoryObservation,
    change_id: &str,
    commits: &[CommitObservation],
) -> CoreResult<JujutsuFacts> {
    let operation = admitted_operation(repository)?;
    let selected: Vec<&CommitObservation> = commits
        .iter()
        .filter(|commit| commit.change_id == change_id)
        .collect();
    if selected.is_empty() {
        return Err(diagnostic(
            "change_missing",
            "jujutsu.change_id",
            "Jujutsu change is not visible at the observed operation",
        ));
    }
    let facts = JujutsuFacts {
        backend: repository.backend,
        operation_identity: ExternalRef::new("jj", "operation", operation)?,
        change_id: ExternalRef::new("jj", "change", change_id)?,
        bookmark: None,
        resolved_commits: selected
            .iter()
            .map(|commit| ExternalRef::new("jj", "commit", &commit.commit))
            .collect::<CoreResult<_>>()?,
        conflicted_bookmark: selected
            .iter()
            .any(|commit| !commit.conflicted_bookmarks.is_empty()),
        divergent_change: selected.len() > 1 || selected.iter().any(|commit| commit.divergent),
        conflicted_commit: selected.iter().any(|commit| commit.conflict),
    };
    admit_jujutsu_facts(&facts)?;
    Ok(facts)
}

fn run(workspace: &Workspace, args: Vec<OsString>) -> CoreResult<Vec<u8>> {
    let failed = || {
        diagnostic(
            "execution_failed",
            "jujutsu.executable",
            "Jujutsu read did not complete",
        )
    };
    if !workspace.executable.is_absolute() || !workspace.workspace.is_absolute() {
        return Err(failed());
    }
    let request = RunRequest {
        command: CommandSpec {
            program: workspace.executable.clone(),
            args,
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: vec![("JJ_CONFIG".into(), JJ_CONFIG_NONE.into())],
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: JJ_TIMEOUT_MS,
            stdin_max_bytes: 1,
            stdout_max_bytes: JJ_STDOUT_BYTES,
            stderr_max_bytes: JJ_STDERR_BYTES,
            poll_interval_ms: JJ_POLL_MS,
            teardown_timeout_ms: JJ_TEARDOWN_MS,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|_| failed())?,
    };
    let output = bounded_exec::run(request).map_err(|_| failed())?;
    if output.exit_code != Some(0) || output.stdout.truncated {
        return Err(failed());
    }
    Ok(output.stdout.bytes)
}

/// Observe and admit an ordered list of exact change IDs at one operation.
/// Every change is admitted before the caller may issue any provider effect.
pub fn observe_changes(workspace: &Workspace, change_ids: &[String]) -> CoreResult<Observation> {
    // Validate input before touching the filesystem or spawning jj.
    change_revset(change_ids)?;
    let repository = observe_repository(&workspace.workspace)?;
    if repository.backend != JujutsuBackend::Git {
        return Err(diagnostic(
            "unsupported_backend",
            "jujutsu.backend",
            "Jujutsu backend is not admitted",
        ));
    }
    let operation = admitted_operation(&repository)?.to_owned();
    let git_store = repository
        .git_store
        .clone()
        .ok_or_else(observation_invalid)?;
    let stdout = run(
        workspace,
        log_args(&workspace.workspace, &operation, change_ids)?,
    )?;
    let commits = parse_log(&stdout)?;
    let mut changes = Vec::with_capacity(change_ids.len());
    for change_id in change_ids {
        let facts = admit_change(&repository, change_id, &commits)?;
        let commit = facts
            .resolved_commits
            .first()
            .map(|commit| commit.native_id.as_str().to_owned())
            .ok_or_else(observation_invalid)?;
        changes.push((change_id.clone(), commit, facts));
    }
    let mut admitted = Vec::with_capacity(changes.len());
    for (change_id, commit, facts) in changes {
        let stdout = run(
            workspace,
            evolog_args(&workspace.workspace, &operation, &commit)?,
        )?;
        let evolog = parse_evolog(&stdout, &commit)?;
        admitted.push(AdmittedChange {
            change_id,
            commit,
            facts,
            evolog,
        });
    }
    Ok(Observation {
        operation,
        git_store,
        changes: admitted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGE: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
    const OTHER: &str = "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt";
    const COMMIT: &str = "123bf0fdc967918ba6dabb3d077b92e6580e2149";
    const COMMIT_B: &str = "29894e5fef59ed8e96fb17c53d0f6d8f24c8a111";

    fn operation() -> String {
        "b7".repeat(OPERATION_ID_HEX / 2)
    }

    fn repository(heads: usize) -> RepositoryObservation {
        let names: Vec<String> = (0..heads).map(|index| format!("{index:0>128}")).collect();
        admit_operation_heads(JujutsuBackend::Git, None, &names).unwrap_or_else(|_| unreachable!())
    }

    fn commit(id: &str, divergent: bool, conflict: bool, bookmarks: &[&str]) -> CommitObservation {
        CommitObservation {
            commit: id.into(),
            change_id: CHANGE.into(),
            divergent,
            conflict,
            conflicted_bookmarks: bookmarks.iter().map(|b| (*b).into()).collect(),
        }
    }

    fn code(result: CoreResult<JujutsuFacts>) -> String {
        result
            .map(|_| String::new())
            .unwrap_or_else(|error| error.code)
    }

    // r[verify kiln.review.jj_intake.admission]
    #[test]
    fn admission_refuses_each_unsafe_fact_with_a_stable_code() {
        let one = repository(1);
        assert!(admit_change(&one, CHANGE, &[commit(COMMIT, false, false, &[])]).is_ok());
        assert_eq!(
            code(admit_change(&repository(0), CHANGE, &[])),
            "operation_missing"
        );
        assert_eq!(
            code(admit_change(&repository(2), CHANGE, &[])),
            "operation_divergent"
        );
        assert_eq!(code(admit_change(&one, CHANGE, &[])), "change_missing");
        assert_eq!(
            code(admit_change(
                &one,
                CHANGE,
                &[
                    commit(COMMIT, true, false, &[]),
                    commit(COMMIT_B, true, false, &[])
                ]
            )),
            "divergent_change"
        );
        assert_eq!(
            code(admit_change(
                &one,
                CHANGE,
                &[commit(COMMIT, false, false, &["bk"])]
            )),
            "conflicted_bookmark"
        );
        assert_eq!(
            code(admit_change(
                &one,
                CHANGE,
                &[commit(COMMIT, false, true, &[])]
            )),
            "conflicted_commit"
        );
        let mut other = one.clone();
        other.backend = JujutsuBackend::Other;
        assert_eq!(
            code(admit_change(
                &other,
                CHANGE,
                &[commit(COMMIT, false, false, &[])]
            )),
            "unsupported_backend"
        );
    }

    // r[verify kiln.review.jj_intake]
    #[test]
    fn revsets_are_built_only_from_full_unique_ids() {
        assert_eq!(
            change_revset(&[CHANGE.into(), OTHER.into()])
                .ok()
                .as_deref(),
            Some(
                "change_id(wnloytsvxwxwxtpqyuwxutmotlmslszv) | change_id(kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt)"
            )
        );
        for bad in [
            vec![],
            vec!["wnloyt".to_owned()],
            vec!["all()".to_owned()],
            vec![CHANGE.to_owned(), CHANGE.to_owned()],
            vec![CHANGE.to_owned(); MAX_CHANGE_IDS + 1],
        ] {
            assert!(change_revset(&bad).is_err());
        }
        assert!(evolog_args(Path::new("/w"), &operation(), "HEAD").is_err());
        let args = log_args(Path::new("/w"), &operation(), &[CHANGE.into()])
            .unwrap_or_else(|_| unreachable!());
        assert!(args.contains(&"--ignore-working-copy".into()));
        assert!(args.contains(&"--at-op".into()));
    }

    #[test]
    fn parsers_accept_the_fixed_templates_and_reject_drift() {
        let line = format!("[\"{COMMIT}\",\"{CHANGE}\",false,true,[\"bk\"]]\n");
        let parsed = parse_log(line.as_bytes()).unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed, vec![commit(COMMIT, false, true, &["bk"])]);
        for bad in [
            "not json\n".to_owned(),
            format!("[\"{COMMIT}\",\"WNLOY\",false,false,[]]\n"),
            format!("[\"HEAD\",\"{CHANGE}\",false,false,[]]\n"),
            format!("[\"{COMMIT}\",\"{CHANGE}\",false]\n"),
        ] {
            assert!(parse_log(bad.as_bytes()).is_err(), "{bad}");
        }
        let op = operation();
        let evolog = format!("[\"{COMMIT}\",[\"{COMMIT_B}\"],\"{op}\"]\n");
        let parsed = parse_evolog(evolog.as_bytes(), COMMIT).unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.predecessors, vec![COMMIT_B.to_owned()]);
        assert_eq!(parsed.operation, Some(op.clone()));
        assert!(parse_evolog(evolog.as_bytes(), COMMIT_B).is_err());
        assert!(parse_evolog(format!("{evolog}{evolog}").as_bytes(), COMMIT).is_err());
        let none = format!("[\"{COMMIT}\",[],null]\n");
        assert!(parse_evolog(none.as_bytes(), COMMIT).is_ok_and(|e| e.operation.is_none()));
        assert!(admit_operation_heads(JujutsuBackend::Git, None, &["../x".into()]).is_err());
    }
}
