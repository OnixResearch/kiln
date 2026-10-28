//! Read-only local source observation for Git, Jujutsu and Pijul repositories.
//!
//! Git and Jujutsu are read through the repository's Git object store. Pijul is
//! read through a separately executed `pijul` program, never a linked library,
//! and only for its current channel state. Nothing here mutates a repository,
//! resolves a remote, or grants CI, gate or release meaning.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bounded_exec::{CommandSpec, EnvironmentMode, Input, RunRequest, TerminationScope};
use bounded_exec_core::{ExecutionLimits, OutcomePolicy};
use git2::{ObjectType, Oid, Repository, RepositoryOpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::api::{decoded, error, json_response, query};
use crate::engine::{ApiResponse, Role};

pub const MAX_SOURCES: usize = 64;
const MAX_POLICY_BYTES: usize = 262_144;
const MAX_QUERY_BYTES: usize = 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_LOG_LIMIT: usize = 30;
const MAX_LOG_LIMIT: usize = 100;
const MAX_TREE_ENTRIES: usize = 512;
const MAX_FILE_BYTES: usize = 262_144;
const MAX_PATH_BYTES: usize = 1024;
const MAX_SUMMARY_CHARS: usize = 256;
const MAX_AUTHOR_CHARS: usize = 256;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_TREE_BYTES: usize = 65_536;
const PIJUL_TIMEOUT_MS: u64 = 15_000;
const PIJUL_POLL_MS: u64 = 25;
const PIJUL_TEARDOWN_MS: u64 = 2_000;
const PIJUL_OUTPUT_BYTES: usize = 1_048_576;
const PIJUL_ERROR_BYTES: usize = 8_192;

const KINDS: [&str; 3] = ["git", "jj", "pijul"];

const NON_CLAIMS: [&str; 3] = [
    "Source views are bounded local reads of one repository; they are not a checkout, build, verification or provenance claim.",
    "Git and Jujutsu views resolve an exact commit in the repository's Git object store; Pijul views cover its current channel state and working copy only.",
    "Browsing never fetches, mutates, signs, publishes or grants CI, gate, acceptance or release authority.",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicy {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: PathBuf,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub sources: Vec<SourcePolicy>,
}

impl Profile {
    // r[impl kiln.dashboard.source.authority]
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-sources.v1" || self.sources.len() > MAX_SOURCES {
            return Err("source policy schema or repository bound rejected".into());
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for source in &self.sources {
            if !crate::profile::identifier(&source.id)
                || source.id.len() > 128
                || source.label.is_empty()
                || source.label.len() > 128
                || source.label.chars().any(char::is_control)
                || !KINDS.contains(&source.kind.as_str())
                || !crate::profile::absolute(&source.path)
                || !ids.insert(&source.id)
                || !paths.insert(&source.path)
            {
                return Err("source repository policy rejected".into());
            }
            match source.kind.as_str() {
                "pijul" => {
                    let channel = source.channel.as_deref().unwrap_or_default();
                    if !crate::profile::identifier(channel) || channel.len() > 128 {
                        return Err("Pijul source requires a bounded channel name".into());
                    }
                    if source
                        .executable
                        .as_deref()
                        .is_some_and(|path| !crate::profile::absolute(path))
                    {
                        return Err("Pijul executable must be an absolute path".into());
                    }
                }
                _ if source.channel.is_some() || source.executable.is_some() => {
                    return Err("Git and Jujutsu sources take no channel or executable".into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_POLICY_BYTES, false)
            .map_err(|_| "source policy unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "source policy malformed")?;
        profile.admit()?;
        Ok(profile)
    }
}

#[derive(Debug)]
pub struct Browser {
    profile: Option<Profile>,
}

impl Browser {
    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self { profile })
    }

    // r[impl kiln.dashboard.source.read] r[impl kiln.dashboard.source.authority]
    pub fn response(&self, method: &str, target: &str, body: &[u8], role: Role) -> ApiResponse {
        match self.read(method, target, body, role) {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            _ => ApiResponse {
                status: 503,
                content_type: "application/json",
                body: br#"{"code":"source_read_unavailable","message":"Source read exceeded its bounds or is unavailable"}"#.to_vec(),
                download_name: None,
            },
        }
    }

    fn read(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if role == Role::Worker {
            return error(403, "worker_scope");
        }
        if method != "GET" {
            return error(405, "sources_read_only");
        }
        if !body.is_empty() {
            return error(400, "unexpected_body");
        }
        let (path, parameters) = match query(target) {
            Ok(value) => value,
            Err(()) => return error(400, "query_invalid"),
        };
        let mut values = BTreeMap::new();
        for (key, value) in parameters {
            match decoded(value) {
                Ok(value)
                    if value.len() <= MAX_QUERY_BYTES && !value.chars().any(char::is_control) =>
                {
                    values.insert(key.to_owned(), value);
                }
                _ => return error(400, "query_invalid"),
            }
        }
        if path == "/api/sources" {
            if !values.is_empty() {
                return error(400, "query_invalid");
            }
            return self.inventory();
        }
        let Some(rest) = path.strip_prefix("/api/sources/") else {
            return error(404, "route_missing");
        };
        let Some((id, leaf)) = rest.split_once('/') else {
            return error(404, "route_missing");
        };
        let allowed: &[&str] = match leaf {
            "log" => &["limit", "cursor"],
            "tree" | "file" => &["revision", "path"],
            _ => return error(404, "route_missing"),
        };
        if values.keys().any(|key| !allowed.contains(&key.as_str())) {
            return error(400, "query_invalid");
        }
        let Some(profile) = &self.profile else {
            return error(404, "source_missing");
        };
        let Some(policy) = profile.sources.iter().find(|row| row.id == id) else {
            return error(404, "source_missing");
        };
        match leaf {
            "log" => self.log(policy, &values),
            "tree" => self.tree(policy, &values),
            _ => self.file(policy, &values),
        }
    }

    fn inventory(&self) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"configured":false,"sources":[],"non_claims":NON_CLAIMS}),
            );
        };
        let sources = profile
            .sources
            .iter()
            .map(|policy| {
                let observed = observe(policy);
                json!({
                    "id": policy.id,
                    "label": policy.label,
                    "kind": policy.kind,
                    "channel": policy.channel,
                    "status": if observed.head.is_some() { "available" } else { "unavailable" },
                    "head": observed.head,
                    "revision_scope": observed.scope,
                    "reason": observed.reason,
                })
            })
            .collect::<Vec<_>>();
        json_response(
            200,
            json!({"configured":true,"sources":sources,"non_claims":NON_CLAIMS}),
        )
    }

    fn log(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let limit = match values.get("limit") {
            None => DEFAULT_LOG_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_LOG_LIMIT).contains(&limit) => limit,
                _ => return error(400, "source_limit_invalid"),
            },
        };
        let offset = match values.get("cursor") {
            None => 0,
            Some(value) => match value.parse::<usize>() {
                Ok(offset) if offset <= 100_000 => offset,
                _ => return error(400, "source_cursor_invalid"),
            },
        };
        match policy.kind.as_str() {
            "pijul" => pijul_log(policy, limit, offset),
            _ => git_log(policy, limit, offset),
        }
    }

    fn tree(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let components = match components(path) {
            Ok(components) => components,
            Err(()) => return error(400, "source_path_invalid"),
        };
        match policy.kind.as_str() {
            "pijul" => pijul_tree(policy, revision, &components),
            _ => git_tree(policy, revision, &components),
        }
    }

    fn file(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let components = match components(path) {
            Ok(components) => components,
            Err(()) => return error(400, "source_path_invalid"),
        };
        if components.is_empty() {
            return error(400, "source_path_invalid");
        }
        match policy.kind.as_str() {
            "pijul" => pijul_file(policy, revision, &components),
            _ => git_file(policy, revision, &components),
        }
    }
}

struct Observation {
    head: Option<String>,
    scope: &'static str,
    reason: Option<&'static str>,
}

fn observe(policy: &SourcePolicy) -> Observation {
    match policy.kind.as_str() {
        "pijul" => match pijul_head(policy) {
            Ok(head) => Observation {
                head: Some(head),
                scope: "current_channel_state",
                reason: None,
            },
            Err(reason) => Observation {
                head: None,
                scope: "current_channel_state",
                reason: Some(reason),
            },
        },
        _ => match open_git(policy).and_then(|repository| head(&repository)) {
            Ok(head) => Observation {
                head: Some(head.to_string()),
                scope: "revision",
                reason: None,
            },
            Err(reason) => Observation {
                head: None,
                scope: "revision",
                reason: Some(reason),
            },
        },
    }
}

fn reject_symlink(path: &Path) -> Result<PathBuf, &'static str> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "source path is unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("source path is not a plain directory");
    }
    Ok(path.to_owned())
}

fn git_directory(policy: &SourcePolicy) -> Result<PathBuf, &'static str> {
    let root = reject_symlink(&policy.path)?;
    let worktree = root.join(".git");
    let worktree_is_directory = std::fs::symlink_metadata(&worktree)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());
    if worktree_is_directory {
        return Ok(worktree);
    }
    if policy.kind == "git" {
        if std::fs::symlink_metadata(root.join("HEAD")).is_ok() {
            return Ok(root);
        }
        return Err("source path is not a Git repository or worktree");
    }
    let backend = root.join(".jj").join("repo").join("store").join("git");
    if std::fs::symlink_metadata(backend.join("HEAD")).is_ok() {
        return Ok(backend);
    }
    Err("source path has no Git-visible Jujutsu store or head")
}

fn open_git(policy: &SourcePolicy) -> Result<Repository, &'static str> {
    let directory = git_directory(policy)?;
    Repository::open_ext(
        &directory,
        RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::NO_DOTGIT,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .map_err(|_| "source repository could not be opened")
}

fn head(repository: &Repository) -> Result<Oid, &'static str> {
    repository
        .head()
        .and_then(|reference| reference.peel_to_commit())
        .map(|commit| commit.id())
        .map_err(|_| "source repository has no resolvable head commit")
}

fn commit<'repo>(
    repository: &'repo Repository,
    revision: &str,
) -> Result<git2::Commit<'repo>, &'static str> {
    if revision.is_empty() {
        let oid = head(repository)?;
        return bounded_commit(repository, oid);
    }
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("revision is not an exact 40-character commit id");
    }
    let oid = Oid::from_str(revision).map_err(|_| "revision is not an exact commit id")?;
    bounded_commit(repository, oid)
}

fn bounded_commit(repository: &Repository, oid: Oid) -> Result<git2::Commit<'_>, &'static str> {
    let (size, kind) = repository
        .odb()
        .and_then(|odb| odb.read_header(oid))
        .map_err(|_| "revision is not an available object")?;
    if kind != ObjectType::Commit || size > MAX_COMMIT_BYTES {
        return Err("revision is not a bounded commit object");
    }
    repository
        .find_commit(oid)
        .map_err(|_| "revision is not an available commit")
}

fn tree<'repo>(
    repository: &'repo Repository,
    commit: &git2::Commit<'repo>,
) -> Result<git2::Tree<'repo>, &'static str> {
    let (size, kind) = repository
        .odb()
        .and_then(|odb| odb.read_header(commit.tree_id()))
        .map_err(|_| "revision tree is unavailable")?;
    if kind != ObjectType::Tree || size > MAX_TREE_BYTES {
        return Err("revision tree exceeds its bound");
    }
    repository
        .find_tree(commit.tree_id())
        .map_err(|_| "revision tree is unavailable")
}

fn components(path: &str) -> Result<Vec<&str>, ()> {
    if path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
        return Err(());
    }
    let mut result = Vec::new();
    if path.is_empty() {
        return Ok(result);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\\') {
            return Err(());
        }
        result.push(part);
    }
    Ok(result)
}

fn brief(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn git_log(policy: &SourcePolicy, limit: usize, offset: usize) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let head = match head(&repository) {
        Ok(head) => head,
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut walk = match repository.revwalk() {
        Ok(walk) => walk,
        Err(_) => return error(503, "source_unavailable"),
    };
    if walk.push(head).is_err() {
        return error(503, "source_unavailable");
    }
    let mut entries = Vec::new();
    let mut total = 0usize;
    for (index, oid) in walk.enumerate() {
        let oid = match oid {
            Ok(oid) => oid,
            Err(_) => return error(503, "source_unavailable"),
        };
        total = index.saturating_add(1);
        if index < offset {
            continue;
        }
        if entries.len() > limit {
            break;
        }
        let commit = match bounded_commit(&repository, oid) {
            Ok(commit) => commit,
            Err(_) => return error(503, "source_unavailable"),
        };
        let author = commit.author();
        entries.push(json!({
            "id": oid.to_string(),
            "summary": brief(commit.summary().unwrap_or(""), MAX_SUMMARY_CHARS),
            "author": brief(
                &format!(
                    "{} <{}>",
                    author.name().unwrap_or(""),
                    author.email().unwrap_or("")
                ),
                MAX_AUTHOR_CHARS,
            ),
            "recorded_at_ms": commit.time().seconds().saturating_mul(1000),
            "parents": commit.parent_ids().map(|id| id.to_string()).collect::<Vec<_>>(),
        }));
    }
    let has_more = entries.len() > limit;
    entries.truncate(limit);
    let next = offset.saturating_add(entries.len());
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "head": head.to_string(),
            "revision_scope": "revision",
            "entries": entries,
            "next_cursor": if has_more { Some(next.to_string()) } else { None },
            "observed_revisions": total,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn git_tree(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut tree = match tree(&repository, &commit) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    for part in components {
        let next = match tree.get_name(part) {
            Some(entry) if entry.kind() == Some(ObjectType::Tree) => entry.id(),
            Some(_) => return error(404, "source_path_missing"),
            None => return error(404, "source_path_missing"),
        };
        tree = match repository.find_tree(next) {
            Ok(tree) => tree,
            Err(_) => return error(503, "source_unavailable"),
        };
    }
    if tree.len() > MAX_TREE_ENTRIES {
        return error(503, "source_tree_bound");
    }
    let mut entries = Vec::new();
    for entry in tree.iter() {
        let name = match entry.name() {
            Some(name) => name,
            None => return error(503, "source_unavailable"),
        };
        let kind = match entry.kind() {
            Some(ObjectType::Blob) if entry.filemode() == 0o120000 => "symlink",
            Some(ObjectType::Blob) => "file",
            Some(ObjectType::Tree) => "directory",
            _ => "submodule",
        };
        let bytes = if kind == "file" {
            repository
                .find_blob(entry.id())
                .map(|blob| blob.size())
                .unwrap_or(0)
        } else {
            0
        };
        entries.push(json!({
            "name": name,
            "path": if components.is_empty() {
                name.to_owned()
            } else {
                format!("{}/{}", components.join("/"), name)
            },
            "kind": kind,
            "mode": format!("{:o}", entry.filemode()),
            "oid": entry.id().to_string(),
            "bytes": bytes,
        }));
    }
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "revision": commit.id().to_string(),
            "revision_scope": "revision",
            "path": components.join("/"),
            "entries": entries,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn git_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let tree = match tree(&repository, &commit) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    let entry = match tree.get_path(Path::new(&components.join("/"))) {
        Ok(entry) => entry,
        Err(_) => return error(404, "source_path_missing"),
    };
    if entry.kind() != Some(ObjectType::Blob) || entry.filemode() == 0o120000 {
        return error(404, "source_path_missing");
    }
    let (size, kind) = match repository.odb().and_then(|odb| odb.read_header(entry.id())) {
        Ok(header) => header,
        Err(_) => return error(503, "source_unavailable"),
    };
    if kind != ObjectType::Blob {
        return error(404, "source_path_missing");
    }
    if size > MAX_FILE_BYTES {
        return error(413, "source_file_too_large");
    }
    let blob = match repository.find_blob(entry.id()) {
        Ok(blob) => blob,
        Err(_) => return error(503, "source_unavailable"),
    };
    let content = blob.content();
    let binary = content.contains(&0) || std::str::from_utf8(content).is_err();
    let mut response = json!({
        "source": policy.id,
        "kind": policy.kind,
        "label": policy.label,
        "revision": commit.id().to_string(),
        "revision_scope": "revision",
        "path": components.join("/"),
        "oid": entry.id().to_string(),
        "bytes": content.len(),
        "binary": binary,
    });
    if !binary {
        if let Ok(text) = std::str::from_utf8(content) {
            if let Some(object) = response.as_object_mut() {
                object.insert("text".into(), Value::String(text.to_owned()));
            }
        }
    }
    if let Some(object) = response.as_object_mut() {
        object.insert("non_claims".into(), json!(NON_CLAIMS));
    }
    json_response(200, response)
}

struct PijulRun {
    stdout: Vec<u8>,
}

fn pijul_run(policy: &SourcePolicy, args: Vec<String>) -> Result<PijulRun, &'static str> {
    let executable = policy
        .executable
        .as_deref()
        .ok_or("no pijul executable is configured")?;
    reject_symlink(&policy.path)?;
    if !policy.path.join(".pijul").is_dir() {
        return Err("source path is not a Pijul repository");
    }
    let limits = ExecutionLimits {
        timeout_ms: PIJUL_TIMEOUT_MS,
        stdin_max_bytes: 1,
        stdout_max_bytes: PIJUL_OUTPUT_BYTES,
        stderr_max_bytes: PIJUL_ERROR_BYTES,
        poll_interval_ms: PIJUL_POLL_MS,
        teardown_timeout_ms: PIJUL_TEARDOWN_MS,
    };
    let outcome = OutcomePolicy::new(vec![0], true, true).map_err(|_| "pijul policy invalid")?;
    let request = RunRequest {
        command: CommandSpec {
            program: executable.to_owned(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits,
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: outcome,
    };
    let output = bounded_exec::run(request).map_err(|_| "pijul execution failed")?;
    if output.exit_code != Some(0) || output.stdout.truncated {
        return Err("pijul read did not complete");
    }
    Ok(PijulRun {
        stdout: output.stdout.bytes,
    })
}

fn pijul_base_args(policy: &SourcePolicy, command: &str) -> Vec<String> {
    vec![
        command.to_owned(),
        "--repository".to_owned(),
        policy.path.display().to_string(),
        "--no-prompt".to_owned(),
    ]
}

fn pijul_head(policy: &SourcePolicy) -> Result<String, &'static str> {
    let mut args = pijul_base_args(policy, "log");
    for flag in ["--hash-only", "--limit", "1"] {
        args.push(flag.to_owned());
    }
    let run = pijul_run(policy, args)?;
    let text = std::str::from_utf8(&run.stdout).map_err(|_| "pijul output is not UTF-8")?;
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .ok_or("pijul channel has no recorded change")
}

fn pijul_log(policy: &SourcePolicy, limit: usize, offset: usize) -> Result<ApiResponse, String> {
    let mut args = pijul_base_args(policy, "log");
    for flag in [
        "--output-format",
        "json",
        "--limit",
        &limit.to_string(),
        "--offset",
        &offset.to_string(),
    ] {
        args.push(flag.to_owned());
    }
    let run = match pijul_run(policy, args) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let rows: Vec<Value> = match serde_json::from_slice(&run.stdout) {
        Ok(rows) => rows,
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut entries = Vec::new();
    for row in rows.iter().take(limit) {
        let id = row.get("hash").and_then(Value::as_str).unwrap_or_default();
        let authors = row
            .get("authors")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        entries.push(json!({
            "id": brief(id, MAX_SUMMARY_CHARS),
            "summary": brief(
                row.get("message").and_then(Value::as_str).unwrap_or_default(),
                MAX_SUMMARY_CHARS,
            ),
            "author": brief(&authors, MAX_AUTHOR_CHARS),
            "recorded_at_ms": row
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_rfc3339_ms),
        }));
    }
    let has_more = rows.len() > entries.len();
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "channel": policy.channel,
            "revision_scope": "current_channel_state",
            "entries": entries,
            "next_cursor": if has_more {
                Some((offset.saturating_add(entries.len())).to_string())
            } else {
                None
            },
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn pijul_tree(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    if !revision.is_empty() {
        return error(400, "source_revision_unsupported");
    }
    let run = match pijul_run(policy, pijul_base_args(policy, "list")) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let text = match std::str::from_utf8(&run.stdout) {
        Ok(text) => text,
        Err(_) => return error(503, "source_unavailable"),
    };
    let prefix = components.join("/");
    let mut seen = BTreeSet::new();
    let mut entries = vec![];
    for line in text.lines() {
        let path = line.trim();
        if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains('\\') {
            continue;
        }
        let parts: Vec<&str> = path.split('/').collect();
        if components.len() >= parts.len() {
            continue;
        }
        if !parts.iter().take(components.len()).eq(components.iter()) {
            continue;
        }
        let Some(name) = parts.get(components.len()) else {
            continue;
        };
        let directory = parts.len() > components.len().saturating_add(1);
        let key = (name.to_string(), directory);
        if !seen.insert(key.clone()) {
            continue;
        }
        if entries.len() >= MAX_TREE_ENTRIES {
            return error(503, "source_tree_bound");
        }
        entries.push(json!({
            "name": name,
            "path": if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            },
            "kind": if directory { "directory" } else { "file" },
            "mode": null,
            "oid": null,
            "bytes": null,
        }));
    }
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "channel": policy.channel,
            "revision": Value::Null,
            "revision_scope": "current_channel_state",
            "path": prefix,
            "entries": entries,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn pijul_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    if !revision.is_empty() {
        return error(400, "source_revision_unsupported");
    }
    let run = match pijul_run(policy, pijul_base_args(policy, "list")) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let text = match std::str::from_utf8(&run.stdout) {
        Ok(text) => text,
        Err(_) => return error(503, "source_unavailable"),
    };
    let relative = components.join("/");
    if !text.lines().any(|line| line.trim() == relative) {
        return error(404, "source_path_missing");
    }
    let mut path = policy.path.clone();
    for part in components {
        path.push(part);
    }
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(_) => return error(404, "source_path_missing"),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return error(404, "source_path_missing");
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return error(413, "source_file_too_large");
    }
    let content = match std::fs::read(&path) {
        Ok(content) => content,
        Err(_) => return error(503, "source_unavailable"),
    };
    let binary = content.contains(&0) || std::str::from_utf8(&content).is_err();
    let mut response = json!({
        "source": policy.id,
        "kind": policy.kind,
        "label": policy.label,
        "channel": policy.channel,
        "revision": Value::Null,
        "revision_scope": "working_copy",
        "path": relative,
        "oid": Value::Null,
        "bytes": content.len(),
        "binary": binary,
    });
    if !binary {
        if let Ok(text) = std::str::from_utf8(&content) {
            if let Some(object) = response.as_object_mut() {
                object.insert("text".into(), Value::String(text.to_owned()));
            }
        }
    }
    if let Some(object) = response.as_object_mut() {
        object.insert("non_claims".into(), json!(NON_CLAIMS));
    }
    json_response(200, response)
}

/// Parse one UTC RFC 3339 timestamp into milliseconds since the Unix epoch.
///
/// Fractional seconds and a `Z` designator are accepted; offsets, non-UTC
/// designs and malformed values yield `None` rather than a guessed instant.
fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || bytes.get(10) != Some(&b'T') {
        return None;
    }
    let number = |start: usize, end: usize| -> Option<i64> {
        let text = value.get(start..end)?;
        if !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse::<i64>().ok()
    };
    let year = number(0, 4)?;
    let month = number(5, 7)?;
    let day = number(8, 10)?;
    let hour = number(11, 13)?;
    let minute = number(14, 16)?;
    let second = number(17, 19)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    if value.get(19..)?.starts_with('.') {
        let rest = value.get(20..)?;
        let digits = rest
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits == 0 {
            return None;
        }
        if value.get(20 + digits..) != Some("Z") {
            return None;
        }
    } else if value.get(19..) != Some("Z") {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(
        days.saturating_mul(86_400)
            .saturating_add(hour * 3_600)
            .saturating_add(minute * 60)
            .saturating_add(second)
            .saturating_mul(1_000),
    )
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_conversion_rejects_non_utc_and_malformed_values() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_ms("2025-12-31T19:10:02.401924062Z"),
            Some(1_767_208_202_000)
        );
        for invalid in [
            "2025-12-31T19:10:02+01:00",
            "2025-12-31 19:10:02Z",
            "2025-13-31T19:10:02Z",
            "2025-12-31T19:10:02",
            "",
        ] {
            assert!(parse_rfc3339_ms(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn path_components_reject_traversal_and_absolute_forms() {
        assert_eq!(components(""), Ok(Vec::new()));
        assert_eq!(components("a/b"), Ok(vec!["a", "b"]));
        for invalid in ["/etc/passwd", "../x", "a//b", "a/./b", "a\\b"] {
            assert!(components(invalid).is_err(), "{invalid}");
        }
    }
}
