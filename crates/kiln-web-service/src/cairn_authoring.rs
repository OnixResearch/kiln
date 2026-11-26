//! r[impl kiln.sdlc.cairn_authoring] Operator-only authoring of explicitly enrolled Cairn worktrees.
//!
//! Cairn 317d3ff648f01a5ea1572e6126f4dd71bc7afa99 owns the mutations. Its CLI
//! has no compare-and-swap option: enroll only exclusive local worktrees, with
//! no independent writers (including Git checkouts, editors and other Cairn
//! processes). Kiln serializes its own operations with a lock on `.cairn`.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use bounded_exec::{
    CommandSpec, Completion, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::{ApiResponse, Role};

pub const ROUTE: &str = "/api/implementation/cairn/authoring";
pub const ARTIFACT_ROUTE: &str = "/api/implementation/cairn/authoring/artifact";
const PROFILE_SCHEMA: &str = "kiln.cairn-authoring-profile.v1";
const SCHEMA: &str = "kiln.cairn-authoring.v1";
const PROFILE_BYTES: usize = 64 * 1024;
const REQUEST_BYTES: usize = 2048;
// Leave headroom for JSON string escaping and the service request bound.
const MAX_ARTIFACT_BYTES: usize = 48 * 1024;
const MAX_ARTIFACT_REQUEST_BYTES: usize = 6 * MAX_ARTIFACT_BYTES + REQUEST_BYTES;
const MAX_ROOTS: usize = 64;
const MAX_NAME_BYTES: usize = 128;
const MAX_TREE_ENTRIES: usize = 8192;
const MAX_TREE_DEPTH: usize = 24;
const MAX_TREE_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const NATIVE_TIMEOUT_MS: u64 = 5_000;
const NATIVE_TEARDOWN_MS: u64 = 1_000;
const PROCESS_POLL_MS: u64 = 10;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootEnrollment {
    pub repository: String,
    pub label: String,
    pub root: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    schema: String,
    cairn_executable: PathBuf,
    roots: Vec<RootEnrollment>,
    #[serde(default)]
    edit_artifacts: bool,
}

#[derive(Debug)]
pub struct Authoring {
    executable: PathBuf,
    roots: BTreeMap<String, RootEnrollment>,
    edit_artifacts: bool,
}

#[derive(Clone, Copy, Debug)]
struct Refusal(u16, &'static str);
const ROOT_INVALID: Refusal = Refusal(503, "cairn_authoring_root_unavailable");
const READ_UNAVAILABLE: Refusal = Refusal(503, "cairn_authoring_read_unavailable");
const STALE: Refusal = Refusal(409, "cairn_authoring_stale_revision");
const INVALID: Refusal = Refusal(400, "cairn_authoring_request_invalid");

fn hex(value: &str) -> bool {
    value.len() == blake3::OUT_LEN * 2
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && value != "archive"
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn directory(path: &Path) -> Result<(), Refusal> {
    if !fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(ROOT_INVALID);
    }
    Ok(())
}

fn native_layout(root: &Path) -> Result<(), Refusal> {
    if !root.is_dir() || !root.canonicalize().is_ok_and(|canonical| canonical == root) {
        return Err(ROOT_INVALID);
    }
    for relative in [".cairn", ".cairn/changes", ".cairn/specs", ".cairn/archive"] {
        directory(&root.join(relative))?;
    }
    Ok(())
}

impl Authoring {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() || !fs::symlink_metadata(path).is_ok_and(|m| m.is_file()) {
            return Err("Cairn authoring profile must be an absolute regular file".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|file| file.take(PROFILE_BYTES as u64 + 1).read_to_end(&mut bytes))
            .map_err(|e| format!("cannot read Cairn authoring profile: {e}"))?;
        if bytes.len() > PROFILE_BYTES {
            return Err("Cairn authoring profile exceeds its size bound".into());
        }
        let profile: Profile = serde_json::from_slice(&bytes)
            .map_err(|e| format!("invalid Cairn authoring profile: {e}"))?;
        if profile.schema != PROFILE_SCHEMA {
            return Err("unrecognized Cairn authoring profile schema".into());
        }
        let mut authoring = Self::new(profile.cairn_executable, profile.roots)?;
        authoring.edit_artifacts = profile.edit_artifacts;
        Ok(authoring)
    }

    pub fn new(cairn_executable: PathBuf, roots: Vec<RootEnrollment>) -> Result<Self, String> {
        if !cairn_executable.is_absolute() || roots.is_empty() || roots.len() > MAX_ROOTS {
            return Err(
                "Cairn authoring requires an absolute executable and a bounded root list".into(),
            );
        }
        // Preserve the configured pathname: the packaged executable is a wrapper
        // and is expected to supply its own pinned Cairn policy.
        if !cairn_executable.is_file() {
            return Err("Cairn authoring executable is not a regular file".into());
        }
        let mut enrolled: BTreeMap<String, RootEnrollment> = BTreeMap::new();
        for mut root in roots {
            if !hex(&root.repository)
                || root.label.trim().is_empty()
                || root.label.len() > MAX_NAME_BYTES
                || root.label.chars().any(char::is_control)
                || !root.root.is_absolute()
            {
                return Err("invalid Cairn authoring root enrollment".into());
            }
            root.root = root
                .root
                .canonicalize()
                .map_err(|e| format!("Cairn root unavailable: {e}"))?;
            if root.root.to_str().is_none() || native_layout(&root.root).is_err() {
                return Err("Cairn authoring requires an existing native .cairn layout".into());
            }
            if enrolled.contains_key(&root.repository)
                || enrolled.values().any(|other| {
                    root.root.starts_with(&other.root) || other.root.starts_with(&root.root)
                })
            {
                return Err(
                    "Cairn authoring roots must not overlap or share a repository identity".into(),
                );
            }
            enrolled.insert(root.repository.clone(), root);
        }
        Ok(Self {
            executable: cairn_executable,
            roots: enrolled,
            edit_artifacts: false,
        })
    }

    fn root(&self, repository: &str) -> Result<&RootEnrollment, Refusal> {
        self.roots
            .get(repository)
            .ok_or(Refusal(404, "cairn_authoring_repository_not_enrolled"))
    }

    fn execute(&self, root: &RootEnrollment, args: &[&str]) -> Option<Native> {
        self.execute_with_input(root, args, None)
    }

    fn execute_with_input(
        &self,
        root: &RootEnrollment,
        args: &[&str],
        content: Option<&str>,
    ) -> Option<Native> {
        let mut native_args = vec![OsString::from("--machine"), "change".into()];
        native_args.extend(args.iter().copied().map(OsString::from));
        native_args.extend([OsString::from("--root"), root.root.as_os_str().into()]);
        let output = bounded_exec::run(RunRequest {
            command: CommandSpec {
                program: self.executable.clone(),
                args: native_args,
                current_dir: PathBuf::from("/"),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: content.map_or(Input::Null, |text| Input::Bytes(text.as_bytes().to_vec())),
            },
            limits: ExecutionLimits {
                timeout_ms: NATIVE_TIMEOUT_MS,
                stdin_max_bytes: content.map_or(1, str::len).max(1),
                stdout_max_bytes: MAX_OUTPUT_BYTES,
                stderr_max_bytes: MAX_OUTPUT_BYTES,
                poll_interval_ms: PROCESS_POLL_MS,
                teardown_timeout_ms: NATIVE_TEARDOWN_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true).ok()?,
        })
        .ok()?;
        if output.completion != Completion::Exited
            || output.stdout.truncated
            || output.stderr.truncated
        {
            return None;
        }
        let envelope: Value = serde_json::from_slice(&output.stdout.bytes).ok()?;
        let success = output.exit_code == Some(0);
        if envelope["schema_version"] != "cairn.cli-machine.v1"
            || envelope["command"] != "change"
            || envelope["ok"].as_bool()? != success
            || (envelope["exit_class"] == "success") != success
        {
            return None;
        }
        Some(Native { success, envelope })
    }

    fn snapshot(&self, root: &RootEnrollment, selected: Option<&str>) -> Result<Snapshot, Refusal> {
        let tree = read_tree(&root.root)?;
        let native = self
            .execute(root, &["graph", "--format", "json"])
            .ok_or(READ_UNAVAILABLE)?;
        let graph = native
            .result(root)
            .filter(|g| g["command"] == "change graph")
            .ok_or(READ_UNAVAILABLE)?;
        let parsed = Graph::parse(graph).ok_or(READ_UNAVAILABLE)?;
        let change = if let Some(selected) = selected {
            // The graph is native authority for active changes; do not let a user
            // invoke `show` on an arbitrary path or a partially created directory.
            let dependencies = parsed
                .nodes
                .get(selected)
                .ok_or(Refusal(404, "cairn_authoring_change_not_found"))?;
            let shown = self
                .execute(root, &["show", selected])
                .ok_or(READ_UNAVAILABLE)?;
            let detail = &shown.result(root).ok_or(READ_UNAVAILABLE)?["change"];
            let shown_dependencies = detail["metadata"].as_object().and_then(|metadata| {
                metadata
                    .get("depends_on")
                    .map_or_else(|| Some(BTreeSet::new()), dependency_set)
            });
            if detail["name"] != selected
                || detail["path"].as_str().map(Path::new)
                    != Some(root.root.join(".cairn/changes").join(selected).as_path())
                || shown_dependencies.as_ref() != Some(dependencies)
            {
                return Err(READ_UNAVAILABLE);
            }
            Some(detail.clone())
        } else {
            None
        };
        if tree != read_tree(&root.root)? {
            return Err(STALE);
        }
        let mut hash = blake3::Hasher::new_derive_key("kiln cairn authoring root revision v1");
        hash.update(root.repository.as_bytes());
        hash_field(&mut hash, root.root.as_os_str().as_encoded_bytes());
        for (path, entry) in &tree {
            hash_field(&mut hash, path.as_encoded_bytes());
            match entry {
                TreeEntry::Directory => {
                    hash.update(b"d");
                }
                TreeEntry::File(digest) => {
                    hash.update(b"f");
                    hash.update(digest.as_bytes());
                }
            }
        }
        Ok(Snapshot {
            revision: hash.finalize().to_hex().to_string(),
            tree,
            graph: graph.clone(),
            parsed,
            change,
        })
    }

    fn mutate(&self, root: &RootEnrollment, request: Mutation) -> Result<Value, Refusal> {
        let _locked = lock_root(root)?;
        let before = self.snapshot(root, None)?;
        if request.expected_revision != before.revision {
            return Err(STALE);
        }
        let action = &request.action;
        let expected = action.admit(&before)?;
        let preview = if let Some((verb, dependency)) = action.edge() {
            let preview = self
                .execute(root, &["depend", verb, action.change(), dependency])
                .ok_or(READ_UNAVAILABLE)?;
            if preview
                .matches_edge(root, action, &before.parsed.nodes, &expected, true)
                .is_none()
            {
                if !preview.success {
                    return Ok(mutation_reply(
                        root,
                        action,
                        "refused",
                        Some(&preview),
                        None,
                        Some(&before),
                    ));
                }
                return Err(READ_UNAVAILABLE);
            }
            Some(preview)
        } else {
            None
        };
        // Native previews do not lock or attest to future state. Repeat the byte
        // comparison immediately before invoking the fixed write verb.
        if read_tree(&root.root)? != before.tree {
            return Err(STALE);
        }
        let native = match action.edge() {
            Some((verb, dependency)) => self.execute(
                root,
                &["depend", verb, action.change(), dependency, "--execute"],
            ),
            None => self.execute(
                root,
                &["create", action.change(), "--profile", "spec-driven"],
            ),
        };
        // A failed/truncated/timed-out invocation may already have written.
        // Never repeat it or report success without independently reading back.
        let (after, shown) = match self.snapshot(root, Some(action.change())) {
            Ok(snapshot) => (Some(snapshot), true),
            Err(_) => (self.snapshot(root, None).ok(), false),
        };
        let reconciled = shown
            && after.as_ref().is_some_and(|after| {
                after.parsed.valid
                    && after.parsed.nodes == expected
                    && after.parsed.archived == before.parsed.archived
                    && after.revision != before.revision
                    && exact_file_effect(action, &before, after, preview.as_ref())
            });
        let complete_receipt = native.as_ref().is_some_and(|native| {
            if action.edge().is_some() {
                native.matches_edge(root, action, &before.parsed.nodes, &expected, false)
                    == preview
                        .as_ref()
                        .and_then(|p| p.envelope["result"]["after_metadata"].as_str())
                    && after.as_ref().is_some_and(|after| {
                        native.matches_manifest(action, &before.tree, &after.tree)
                    })
            } else {
                native.result(root).is_some_and(|result| {
                    result["command"] == "change_create"
                        && result["change"] == action.change()
                        && result["workflow_profile"] == "spec-driven"
                        && result["mutated"] == true
                        && receipt_hash(result)
                })
            }
        });
        let outcome = match (reconciled, complete_receipt) {
            (true, true) => "applied",
            (true, false) => "reconciled",
            (false, _) => "unknown",
        };
        Ok(mutation_reply(
            root,
            action,
            outcome,
            preview.as_ref(),
            native.as_ref(),
            after.as_ref(),
        ))
    }

    fn artifact_view(
        &self,
        root: &RootEnrollment,
        change: &str,
        artifact: Artifact,
    ) -> Result<Value, Refusal> {
        self.require_artifacts()?;
        let _locked = lock_root(root)?;
        let snapshot = self.snapshot(root, Some(change))?;
        let show = self.read_artifact(root, &snapshot, change, artifact)?;
        Ok(
            json!({"schema": SCHEMA, "repository": root.repository, "revision": snapshot.revision, "artifact": show}),
        )
    }

    fn require_artifacts(&self) -> Result<(), Refusal> {
        if self.edit_artifacts {
            Ok(())
        } else {
            Err(Refusal(503, "cairn_authoring_artifact_edit_not_configured"))
        }
    }

    fn read_artifact(
        &self,
        root: &RootEnrollment,
        snapshot: &Snapshot,
        change: &str,
        artifact: Artifact,
    ) -> Result<Value, Refusal> {
        let path = artifact.path(change);
        let native = self
            .execute(root, &["artifact", "show", change, artifact.id()])
            .ok_or(READ_UNAVAILABLE)?;
        let value = native.envelope.get("result").ok_or(READ_UNAVAILABLE)?;
        let content = value["content"].as_str().ok_or(READ_UNAVAILABLE)?;
        let hash = blake3::hash(content.as_bytes());
        if !native.success
            || value["schema_version"] != "cairn.change-artifact-show.v1"
            || value["change"] != change
            || value["artifact"] != artifact.id()
            || value["path"] != path
            || value["content_blake3"].as_str() != Some(hash.to_hex().as_str())
            || snapshot.tree.get(Path::new(&path).as_os_str()) != Some(&TreeEntry::File(hash))
            || content.len() > MAX_ARTIFACT_BYTES
        {
            return Err(READ_UNAVAILABLE);
        }
        Ok(value.clone())
    }

    fn replace_artifact(
        &self,
        root: &RootEnrollment,
        edit: ArtifactEdit,
    ) -> Result<Value, Refusal> {
        self.require_artifacts()?;
        let _locked = lock_root(root)?;
        let before = self.snapshot(root, Some(&edit.change))?;
        if before.revision != edit.expected_revision {
            return Err(STALE);
        }
        let path = edit.artifact.path(&edit.change);
        let key = Path::new(&path).as_os_str();
        let Some(TreeEntry::File(current)) = before.tree.get(key) else {
            return Err(ROOT_INVALID);
        };
        if current.to_hex().as_str() != edit.expected_content_blake3 {
            return Err(STALE);
        }
        let desired = blake3::hash(edit.content.as_bytes());
        if &desired == current {
            return Err(Refusal(409, "cairn_authoring_artifact_unchanged"));
        }
        let native = self.execute_with_input(
            root,
            &[
                "artifact",
                "replace",
                &edit.change,
                edit.artifact.id(),
                "--expected-content-blake3",
                &edit.expected_content_blake3,
                "--execute",
            ],
            Some(&edit.content),
        );
        let after = self.snapshot(root, Some(&edit.change)).ok();
        let observed = after.as_ref().filter(|after| {
            after.tree.get(key) == Some(&TreeEntry::File(desired))
                && before
                    .tree
                    .iter()
                    .filter(|(p, _)| p.as_os_str() != key)
                    .eq(after.tree.iter().filter(|(p, _)| p.as_os_str() != key))
        });
        let acknowledged = native.as_ref().is_some_and(|n| {
            let result = &n.envelope["result"];
            n.success
                && result["schema_version"] == "cairn.change-artifact-replace.v1"
                && result["change"] == edit.change
                && result["artifact"] == edit.artifact.id()
                && result["path"] == path
                && result["previous_content_blake3"] == edit.expected_content_blake3
                && result["content_blake3"].as_str() == Some(desired.to_hex().as_str())
        });
        let outcome = match (observed.is_some(), acknowledged) {
            (true, true) => "applied",
            (true, false) => "reconciled",
            (false, _) => "unknown",
        };
        Ok(
            json!({"schema": SCHEMA, "repository": root.repository, "change": edit.change,
            "artifact": edit.artifact.id(), "outcome": outcome,
            "revision": observed.map(|s| &s.revision),
            "content_blake3": observed.map(|_| desired.to_hex().to_string()),
            "provider": native.map(|n| n.envelope)}),
        )
    }
}

fn lock_root(root: &RootEnrollment) -> Result<File, Refusal> {
    native_layout(&root.root)?;
    let lock = File::open(root.root.join(".cairn")).map_err(|_| ROOT_INVALID)?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .map_err(|_| Refusal(409, "cairn_authoring_root_busy"))?;
    Ok(lock)
}

#[derive(PartialEq, Eq)]
enum TreeEntry {
    Directory,
    File(blake3::Hash),
}

type Tree = BTreeMap<OsString, TreeEntry>;
type Dependencies = BTreeMap<String, BTreeSet<String>>;

fn hash_field(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

// Bounded content identity, not a VCS revision. Include byte-only edits that
// Cairn's graph/show projections would miss. The provider reads only archive
// entry names/types for its graph, not historical artifact trees.
fn read_tree(root: &Path) -> Result<Tree, Refusal> {
    native_layout(root)?;
    fn visit(
        root: &Path,
        relative: &Path,
        depth: usize,
        tree: &mut Tree,
        remaining: &mut usize,
    ) -> io::Result<()> {
        if tree.len() >= MAX_TREE_ENTRIES || depth > MAX_TREE_DEPTH {
            return Err(io::Error::other(
                "Cairn authoring tree exceeds its entry/depth limit",
            ));
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        let digest = if metadata.is_dir() {
            TreeEntry::Directory
        } else if metadata.is_file() {
            // Native dependency writes use fs::write; do not permit a second
            // path to alias the file outside its enrolled change directory.
            #[cfg(unix)]
            if std::os::unix::fs::MetadataExt::nlink(&metadata) != 1 {
                return Err(io::Error::other("hard links are not admitted"));
            }
            let limit = (*remaining).min(MAX_FILE_BYTES);
            if metadata.len() > limit as u64 {
                return Err(io::Error::other(
                    "Cairn authoring tree exceeds its byte limit",
                ));
            }
            let mut reader = File::open(&path)?.take(limit as u64 + 1);
            let mut hash = blake3::Hasher::new();
            let size = io::copy(&mut reader, &mut hash)? as usize;
            if size > limit {
                return Err(io::Error::other(
                    "Cairn authoring tree exceeds its byte limit",
                ));
            }
            *remaining -= size;
            TreeEntry::File(hash.finalize())
        } else {
            return Err(io::Error::other(
                "special files and symlinks are not admitted",
            ));
        };
        tree.insert(relative.as_os_str().to_owned(), digest);
        if metadata.is_dir() && relative.parent() != Some(Path::new(".cairn/archive")) {
            for entry in fs::read_dir(path)? {
                visit(
                    root,
                    &relative.join(entry?.file_name()),
                    depth + 1,
                    tree,
                    remaining,
                )?;
            }
        }
        Ok(())
    }
    let mut tree = Tree::new();
    let mut remaining = MAX_TREE_BYTES;
    visit(root, Path::new(".cairn"), 0, &mut tree, &mut remaining).map_err(|_| ROOT_INVALID)?;
    Ok(tree)
}

struct Native {
    success: bool,
    envelope: Value,
}

impl Native {
    fn result(&self, root: &RootEnrollment) -> Option<&Value> {
        let result = &self.envelope["result"];
        (self.success
            && result["layout"] == "cairn"
            && result["root"].as_str().map(Path::new) == Some(root.root.as_path()))
        .then_some(result)
    }

    fn matches_edge(
        &self,
        root: &RootEnrollment,
        action: &Action,
        before: &Dependencies,
        after: &Dependencies,
        preview: bool,
    ) -> Option<&str> {
        let result = self.result(root)?;
        let (verb, dependency) = action.edge()?;
        let change = action.change();
        if result["command"] != format!("change depend {verb}")
            || result["change"] != change
            || result["dependency"] != dependency
            || result["metadata_path"] != format!(".cairn/changes/{change}/metadata.json")
            || result["valid"] != true
            || result["changed"] != true
            || result["dry_run"] != preview
            || result["mutated"].as_bool() != Some(!preview)
            || !receipt_hash(result)
            || dependency_set(&result["before"]).as_ref() != before.get(change)
            || dependency_set(&result["after"]).as_ref() != after.get(change)
        {
            return None;
        }
        result["after_metadata"].as_str()
    }

    fn matches_manifest(&self, action: &Action, before: &Tree, after: &Tree) -> bool {
        let path = format!(".cairn/changes/{}/metadata.json", action.change());
        let portable = format!("./{path}");
        let key = OsString::from(path);
        let manifest = &self.envelope["result"]["mutation_manifest"];
        let matches = |side: &Value, observed: Option<&TreeEntry>| {
            let Some(entries) = side["entries"]
                .as_array()
                .filter(|entries| entries.len() == 1)
            else {
                return false;
            };
            let entry = &entries[0];
            receipt_hash_field(side, "manifest_hash")
                && entry["path"] == portable
                && match observed {
                    Some(TreeEntry::File(digest)) => {
                        entry["exists"] == true
                            && entry["content_hash"].as_str() == Some(digest.to_hex().as_str())
                    }
                    None => entry["exists"] == false && entry["content_hash"].is_null(),
                    Some(TreeEntry::Directory) => false,
                }
        };
        manifest["kind"] == "change_dependency_metadata"
            && receipt_hash_field(manifest, "manifest_hash")
            && matches(&manifest["before"], before.get(&key))
            && matches(&manifest["after"], after.get(&key))
    }
}

fn receipt_hash(result: &Value) -> bool {
    receipt_hash_field(result, "receipt_hash")
}

fn receipt_hash_field(result: &Value, key: &str) -> bool {
    result[key].as_str().is_some_and(hex)
}

fn dependency_set(value: &Value) -> Option<BTreeSet<String>> {
    let mut dependencies = BTreeSet::new();
    for value in value.as_array()? {
        let dependency = value.as_str()?;
        if !name(dependency) || !dependencies.insert(dependency.to_owned()) {
            return None;
        }
    }
    Some(dependencies)
}

struct Graph {
    valid: bool,
    nodes: Dependencies,
    archived: BTreeSet<String>,
}

impl Graph {
    fn parse(value: &Value) -> Option<Self> {
        let mut nodes = Dependencies::new();
        for node in value["nodes"].as_array()? {
            let id = node["name"].as_str()?;
            if !name(id)
                || nodes
                    .insert(id.to_owned(), dependency_set(&node["dependencies"])?)
                    .is_some()
            {
                return None;
            }
        }
        if value["active_change_count"].as_u64()? != nodes.len() as u64 {
            return None;
        }
        let archived = dependency_set(&value["archived_changes"])?;
        Some(Self {
            valid: value["valid"].as_bool()?,
            nodes,
            archived,
        })
    }
}

struct Snapshot {
    revision: String,
    tree: Tree,
    graph: Value,
    parsed: Graph,
    change: Option<Value>,
}

impl Snapshot {
    fn view(&self) -> Value {
        json!({"revision": self.revision, "graph": self.graph, "change": self.change})
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Artifact {
    Proposal,
    Design,
    Tasks,
}

impl Artifact {
    fn id(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Design => "design",
            Self::Tasks => "tasks",
        }
    }

    fn path(self, change: &str) -> String {
        format!(".cairn/changes/{change}/{}.md", self.id())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactEdit {
    repository: String,
    expected_revision: String,
    change: String,
    artifact: Artifact,
    expected_content_blake3: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    repository: String,
    expected_revision: String,
    action: Action,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Create { change: String },
    AddDependency { change: String, dependency: String },
    RemoveDependency { change: String, dependency: String },
}

impl Action {
    fn change(&self) -> &str {
        match self {
            Self::Create { change }
            | Self::AddDependency { change, .. }
            | Self::RemoveDependency { change, .. } => change,
        }
    }

    fn edge(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::AddDependency { dependency, .. } => Some(("add", dependency)),
            Self::RemoveDependency { dependency, .. } => Some(("remove", dependency)),
            Self::Create { .. } => None,
        }
    }

    fn admit(&self, snapshot: &Snapshot) -> Result<Dependencies, Refusal> {
        let change = self.change();
        let mut expected = snapshot.parsed.nodes.clone();
        let Some((verb, dependency)) = self.edge() else {
            if expected.contains_key(change)
                || snapshot.parsed.archived.contains(change)
                || snapshot
                    .tree
                    .contains_key(Path::new(".cairn/changes").join(change).as_os_str())
            {
                return Err(Refusal(409, "cairn_authoring_change_exists"));
            }
            if !snapshot.parsed.valid {
                return Err(Refusal(409, "cairn_authoring_graph_invalid"));
            }
            expected.insert(change.to_owned(), BTreeSet::new());
            return Ok(expected);
        };
        if change == dependency {
            return Err(Refusal(409, "cairn_authoring_self_dependency"));
        }
        if verb == "add" {
            if !snapshot.parsed.valid
                || !expected.contains_key(dependency)
                    && !snapshot.parsed.archived.contains(dependency)
            {
                return Err(Refusal(409, "cairn_authoring_invalid_prerequisite"));
            }
            if reaches(&expected, dependency, change) {
                return Err(Refusal(409, "cairn_authoring_dependency_cycle"));
            }
        }
        let dependencies = expected
            .get_mut(change)
            .ok_or(Refusal(404, "cairn_authoring_change_not_found"))?;
        if snapshot
            .tree
            .get(Path::new(".cairn/changes").join(change).as_os_str())
            != Some(&TreeEntry::Directory)
        {
            return Err(ROOT_INVALID);
        }
        let changed = if verb == "add" {
            dependencies.insert(dependency.to_owned())
        } else {
            dependencies.remove(dependency)
        };
        if !changed {
            return Err(Refusal(409, "cairn_authoring_edge_unchanged"));
        }
        Ok(expected)
    }
}

fn reaches(graph: &Dependencies, start: &str, target: &str) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if node == target {
            return true;
        }
        if seen.insert(node) {
            pending.extend(graph.get(node).into_iter().flatten().map(String::as_str));
        }
    }
    false
}

fn exact_file_effect(
    action: &Action,
    before: &Snapshot,
    after: &Snapshot,
    preview: Option<&Native>,
) -> bool {
    let change = Path::new(".cairn/changes").join(action.change());
    let edited = change.join("metadata.json");
    let is_affected = |path: &OsString| {
        if action.edge().is_some() {
            path == edited.as_os_str()
        } else {
            Path::new(path).starts_with(&change)
        }
    };
    if !before
        .tree
        .iter()
        .filter(|(p, _)| !is_affected(p))
        .eq(after.tree.iter().filter(|(p, _)| !is_affected(p)))
    {
        return false;
    }
    if action.edge().is_some() {
        return preview
            .and_then(|p| p.envelope["result"]["after_metadata"].as_str())
            .is_some_and(|bytes| {
                after.tree.get(edited.as_os_str())
                    == Some(&TreeEntry::File(blake3::hash(bytes.as_bytes())))
            });
    }
    // The packaged producer's spec-driven scaffold is fixed at this pin.
    let scaffold = [
        "",
        "metadata.json",
        "proposal.md",
        "design.md",
        "tasks.md",
        "specs",
    ];
    after.tree.keys().filter(|p| is_affected(p)).count() == scaffold.len()
        && scaffold.into_iter().all(|path| {
            let key = if path.is_empty() {
                change.clone()
            } else {
                change.join(path)
            };
            after.tree.get(key.as_os_str()).is_some_and(|entry| {
                matches!(entry, TreeEntry::Directory) == (path.is_empty() || path == "specs")
            })
        })
        && after.change.as_ref().is_some_and(|c| {
            c["has_design"] == true && c["has_proposal"] == true && c["has_tasks"] == true
        })
}

fn mutation_reply(
    root: &RootEnrollment,
    action: &Action,
    outcome: &str,
    preview: Option<&Native>,
    native: Option<&Native>,
    snapshot: Option<&Snapshot>,
) -> Value {
    json!({
        "schema": SCHEMA, "repository": root.repository, "action": action,
        "outcome": outcome, "provider": native.map(|value| &value.envelope),
        "preview": preview.map(|value| &value.envelope), "snapshot": snapshot.map(Snapshot::view),
    })
}

fn json_response(status: u16, value: Value) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: value.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

pub fn response(
    authoring: Option<&Authoring>,
    method: &str,
    target: &str,
    body: &[u8],
    role: Role,
    restricted_projection: bool,
) -> ApiResponse {
    let result = (|| {
        if role != Role::Operator || restricted_projection {
            return Err(Refusal(403, "cairn_authoring_forbidden"));
        }
        if !matches!(method, "GET" | "POST") {
            return Err(Refusal(405, "cairn_authoring_method_denied"));
        }
        let authoring = authoring.ok_or(Refusal(503, "cairn_authoring_not_configured"))?;
        if target == ARTIFACT_ROUTE && method == "POST" {
            if body.len() > MAX_ARTIFACT_REQUEST_BYTES {
                return Err(Refusal(413, "cairn_authoring_request_too_large"));
            }
            let edit: ArtifactEdit = serde_json::from_slice(body).map_err(|_| INVALID)?;
            if !hex(&edit.repository)
                || !hex(&edit.expected_revision)
                || !hex(&edit.expected_content_blake3)
                || !name(&edit.change)
                || edit.content.len() > MAX_ARTIFACT_BYTES
            {
                return Err(INVALID);
            }
            return authoring.replace_artifact(authoring.root(&edit.repository)?, edit);
        }
        if method == "GET"
            && body.is_empty()
            && let Some(query) = target
                .strip_prefix(ARTIFACT_ROUTE)
                .and_then(|s| s.strip_prefix("?repository="))
        {
            let (repository, tail) = query.split_once("&change=").ok_or(INVALID)?;
            let (change, artifact) = tail.split_once("&artifact=").ok_or(INVALID)?;
            if !hex(repository) || !name(change) {
                return Err(INVALID);
            }
            let artifact = match artifact {
                "proposal" => Artifact::Proposal,
                "design" => Artifact::Design,
                "tasks" => Artifact::Tasks,
                _ => return Err(INVALID),
            };
            return authoring.artifact_view(authoring.root(repository)?, change, artifact);
        }
        match method {
            "GET" if target == ROUTE && body.is_empty() => Ok(json!({
                "schema": SCHEMA,
                "roots": authoring.roots.values().map(|root| json!({"repository": root.repository, "label": root.label})).collect::<Vec<_>>(),
                "capabilities": {"create": true, "edit_artifacts": authoring.edit_artifacts, "edit_dependencies": true},
            })),
            "GET" if body.is_empty() => {
                let query = target
                    .strip_prefix(ROUTE)
                    .and_then(|s| s.strip_prefix("?repository="))
                    .ok_or(INVALID)?;
                let (repository, selected) = query
                    .split_once("&change=")
                    .map_or((query, None), |(r, c)| (r, Some(c)));
                if !hex(repository) || selected.is_some_and(|change| !name(change)) {
                    return Err(INVALID);
                }
                let root = authoring.root(repository)?;
                let _locked = lock_root(root)?;
                let snapshot = authoring.snapshot(root, selected)?;
                Ok(json!({"schema": SCHEMA, "repository": repository, "snapshot": snapshot.view()}))
            }
            "POST" if target == ROUTE => {
                if body.len() > REQUEST_BYTES {
                    return Err(Refusal(413, "cairn_authoring_request_too_large"));
                }
                let mutation: Mutation = serde_json::from_slice(body).map_err(|_| INVALID)?;
                if !hex(&mutation.repository)
                    || !hex(&mutation.expected_revision)
                    || !name(mutation.action.change())
                    || mutation
                        .action
                        .edge()
                        .is_some_and(|(_, dependency)| !name(dependency))
                {
                    return Err(INVALID);
                }
                authoring.mutate(authoring.root(&mutation.repository)?, mutation)
            }
            _ => Err(INVALID),
        }
    })();
    match result {
        Ok(value) => json_response(
            if value["outcome"] == "refused" {
                409
            } else {
                200
            },
            value,
        ),
        Err(Refusal(status, code)) => json_response(
            status,
            json!({"code": code, "message": "Cairn authoring request could not be admitted"}),
        ),
    }
}

#[cfg(test)]
mod tests;
