//! Native Git framing only: no Cairn reducer, signer, ref write or network transport.
use std::collections::{BTreeMap, BTreeSet};

use git2::{ObjectType, Oid, Repository, RepositoryOpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{MAX_OBJECTS, MAX_REF_TIPS, RepositoryPolicy};

const MAX_REFS: usize = 16_384;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_TREE_BYTES: usize = 65_536;
const MAX_MANIFEST_BYTES: usize = 4096;
const MAX_ACTION_BYTES: usize = 262_144;
const MAX_HISTORY_BYTES: usize = 1_048_576;
const MAX_OPERATIONS: usize = 128;
const MAX_PARENTS: usize = 128;
const MAX_TREE_ENTRIES: usize = 256;
const MAX_ACTIONS: usize = 128;
const MAX_TITLE_CHARS: usize = 256;
const MAX_EXCERPT_CHARS: usize = 180;
const MAX_EXCERPT_BYTES: usize = 8192;
const BLOB_MODE: i32 = 0o100644;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct RefTip {
    reference: String,
    operation_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct Object {
    pub id: String,
    pub repository: String,
    pub repository_label: String,
    pub repository_id: String,
    pub type_name: String,
    pub object_id: String,
    pub title: String,
    pub excerpt: String,
    pub author: String,
    pub recorded_at_ms: i64,
    pub ref_tips: Vec<RefTip>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    #[serde(alias = "typename")]
    type_name: String,
    #[serde(default = "manifest_version")]
    version: usize,
}
fn manifest_version() -> usize {
    1
}

pub(super) fn is_oid(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn open(policy: &RepositoryPolicy) -> Result<Repository, ()> {
    let metadata = std::fs::symlink_metadata(&policy.git_directory).map_err(|_| ())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(());
    }
    Repository::open_ext(
        &policy.git_directory,
        RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::NO_DOTGIT,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .map_err(|_| ())
}

fn commit(repository: &Repository, oid: Oid) -> Result<git2::Commit<'_>, ()> {
    let (size, kind) = repository
        .odb()
        .map_err(|_| ())?
        .read_header(oid)
        .map_err(|_| ())?;
    if kind != ObjectType::Commit || size > MAX_COMMIT_BYTES {
        return Err(());
    }
    repository.find_commit(oid).map_err(|_| ())
}

fn tree<'repo>(
    repository: &'repo Repository,
    commit: &git2::Commit<'repo>,
) -> Result<git2::Tree<'repo>, ()> {
    let (size, kind) = repository
        .odb()
        .map_err(|_| ())?
        .read_header(commit.tree_id())
        .map_err(|_| ())?;
    if kind != ObjectType::Tree || size > MAX_TREE_BYTES {
        return Err(());
    }
    repository.find_tree(commit.tree_id()).map_err(|_| ())
}

fn blob<'repo>(
    repository: &'repo Repository,
    entry: &git2::TreeEntry<'_>,
    maximum: usize,
) -> Result<git2::Blob<'repo>, ()> {
    if entry.filemode() != BLOB_MODE {
        return Err(());
    }
    let (size, kind) = repository
        .odb()
        .map_err(|_| ())?
        .read_header(entry.id())
        .map_err(|_| ())?;
    if kind != ObjectType::Blob || size > maximum {
        return Err(());
    }
    repository.find_blob(entry.id()).map_err(|_| ())
}

fn manifest(repository: &Repository, tree: &git2::Tree<'_>, type_name: &str) -> Result<(), ()> {
    if tree.len() > MAX_TREE_ENTRIES {
        return Err(());
    }
    let entry = tree.get_name("manifest").ok_or(())?;
    let bytes = blob(repository, &entry, MAX_MANIFEST_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(bytes.content()).map_err(|_| ())?;
    if manifest.type_name != type_name || manifest.version != 1 {
        return Err(());
    }
    Ok(())
}

fn object_ref(name: &str) -> Result<Option<(&str, &str)>, ()> {
    let rest = if let Some(rest) = name.strip_prefix("refs/cobs/") {
        rest
    } else if let Some(rest) = name.strip_prefix("refs/namespaces/") {
        let Some((namespace, rest)) = rest.split_once("/refs/cobs/") else {
            return Ok(None);
        };
        if namespace.is_empty() || namespace.contains('/') {
            return Err(());
        }
        rest
    } else {
        return Ok(None);
    };
    let Some((type_name, object)) = rest.split_once('/') else {
        return Err(());
    };
    if !super::supported_type(type_name) {
        return Ok(None);
    }
    if !is_oid(object) {
        return Err(());
    }
    Ok(Some((type_name, object)))
}

fn scan(repository: &Repository) -> Result<BTreeMap<(String, String), Vec<RefTip>>, ()> {
    let mut objects: BTreeMap<(String, String), Vec<RefTip>> = BTreeMap::new();
    for (index, reference) in repository.references().map_err(|_| ())?.enumerate() {
        if index >= MAX_REFS {
            return Err(());
        }
        let reference = reference.map_err(|_| ())?;
        let name = reference.name().ok_or(())?;
        let Some((type_name, object_id)) = object_ref(name)? else {
            continue;
        };
        let target = reference.target().ok_or(())?;
        let tip = commit(repository, target)?;
        manifest(repository, &tree(repository, &tip)?, type_name)?;
        let tips = objects
            .entry((type_name.to_owned(), object_id.to_owned()))
            .or_default();
        if tips.len() >= MAX_REF_TIPS {
            return Err(());
        }
        tips.push(RefTip {
            reference: name.to_owned(),
            operation_id: target.to_string(),
        });
        if objects.len() > MAX_OBJECTS {
            return Err(());
        }
    }
    for tips in objects.values_mut() {
        tips.sort_by(|left, right| left.reference.cmp(&right.reference));
    }
    Ok(objects)
}

/// First recorded free-text field of action zero.
///
/// Radicle COBs carry generic commit summaries ("Create issue"), so the list
/// shows the object's own recorded text instead of inventing a title. Absent or
/// malformed payloads yield an empty excerpt; nothing is synthesized.
fn excerpt(repository: &Repository, tree: &git2::Tree<'_>) -> String {
    const FIELDS: [&str; 6] = ["title", "body", "description", "message", "summary", "name"];
    let Some(entry) = tree.get_name("0") else {
        return String::new();
    };
    let Ok(bytes) = blob(repository, &entry, MAX_EXCERPT_BYTES) else {
        return String::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(bytes.content()) else {
        return String::new();
    };
    let mut candidates = vec![&value];
    for key in ["state", "payload", "object"] {
        if let Some(nested) = value.get(key) {
            candidates.push(nested);
        }
    }
    for field in FIELDS {
        for candidate in &candidates {
            let Some(text) = candidate.get(field).and_then(Value::as_str) else {
                continue;
            };
            let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() {
                continue;
            }
            return normalized.chars().take(MAX_EXCERPT_CHARS).collect();
        }
    }
    String::new()
}

fn row(
    repository: &Repository,
    policy: &RepositoryPolicy,
    type_name: String,
    object_id: String,
    ref_tips: Vec<RefTip>,
) -> Result<Object, ()> {
    let root = commit(repository, Oid::from_str(&object_id).map_err(|_| ())?)?;
    let tree = tree(repository, &root)?;
    manifest(repository, &tree, &type_name)?;
    let title = root
        .summary()
        .ok_or(())?
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    let author = root.author();
    let author = format!(
        "{} <{}>",
        author.name().unwrap_or(""),
        author.email().unwrap_or("")
    )
    .chars()
    .take(MAX_TITLE_CHARS)
    .collect();
    let recorded_at_ms = root.time().seconds().saturating_mul(1000);
    let excerpt = excerpt(repository, &tree);
    let id = crate::engine::identity(&(
        "kiln.web-cob-object.v1",
        &policy.repository_id,
        &type_name,
        &object_id,
    ))
    .map_err(|_| ())?
    .as_str()
    .to_owned();
    Ok(Object {
        id,
        repository: policy.id.clone(),
        repository_label: policy.label.clone(),
        repository_id: policy.repository_id.clone(),
        type_name,
        object_id,
        title,
        excerpt,
        author,
        recorded_at_ms,
        ref_tips,
    })
}

// r[impl kiln.dashboard.cobs.read]
pub(super) fn inventory(policy: &RepositoryPolicy) -> Result<Vec<Object>, ()> {
    let repository = open(policy)?;
    scan(&repository)?
        .into_iter()
        .map(|((type_name, object_id), tips)| row(&repository, policy, type_name, object_id, tips))
        .collect()
}

fn actions(
    repository: &Repository,
    tree: &git2::Tree<'_>,
    retained: &mut usize,
) -> Result<Vec<Value>, ()> {
    let mut actions = BTreeMap::new();
    for entry in tree.iter() {
        let name = entry.name().ok_or(())?;
        if !name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let index = name.parse::<usize>().map_err(|_| ())?;
        if index >= MAX_ACTIONS || (name.len() > 1 && name.starts_with('0')) {
            return Err(());
        }
        let bytes = blob(repository, &entry, MAX_ACTION_BYTES)?;
        *retained = retained.checked_add(bytes.size()).ok_or(())?;
        if *retained > MAX_HISTORY_BYTES {
            return Err(());
        }
        let value: Value = serde_json::from_slice(bytes.content()).map_err(|_| ())?;
        actions.insert(index, json!({"index":index,"blob_id":entry.id().to_string(),"bytes":bytes.size(),"value":value}));
    }
    if actions.is_empty() || actions.keys().copied().ne(0..actions.len()) {
        return Err(());
    }
    Ok(actions.into_values().collect())
}

fn history_parents(commit: &git2::Commit<'_>) -> Result<Vec<Oid>, ()> {
    if commit.parent_count() > MAX_PARENTS {
        return Err(());
    }
    let message = commit.message().ok_or(())?;
    let trailers = git2::message_trailers_strs(message).map_err(|_| ())?;
    let mut related = BTreeSet::new();
    for (key, value) in trailers.iter() {
        if key == "Rad-Resource" || key == "Rad-Related" {
            related.insert(Oid::from_str(value).map_err(|_| ())?);
        }
    }
    Ok(commit
        .parent_ids()
        .filter(|parent| !related.contains(parent))
        .collect())
}

// r[impl kiln.dashboard.cobs.read] r[impl kiln.dashboard.cobs.authority]
pub(super) fn detail(
    policy: &RepositoryPolicy,
    type_name: &str,
    object_id: &str,
) -> Result<Option<Value>, ()> {
    let repository = open(policy)?;
    let before = scan(&repository)?;
    let key = (type_name.to_owned(), object_id.to_owned());
    let Some(tips) = before.get(&key) else {
        return Ok(None);
    };
    let object = row(
        &repository,
        policy,
        key.0.clone(),
        key.1.clone(),
        tips.clone(),
    )?;
    let root = Oid::from_str(object_id).map_err(|_| ())?;
    let mut pending = tips
        .iter()
        .map(|tip| Oid::from_str(&tip.operation_id).map_err(|_| ()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut visited = BTreeSet::new();
    let mut operations = BTreeMap::new();
    let mut retained = 0;
    while let Some(oid) = pending.pop() {
        if !visited.insert(oid) {
            continue;
        }
        if visited.len() > MAX_OPERATIONS {
            return Err(());
        }
        let commit = commit(&repository, oid)?;
        let tree = tree(&repository, &commit)?;
        manifest(&repository, &tree, type_name)?;
        let contents = actions(&repository, &tree, &mut retained)?;
        let parents = history_parents(&commit)?;
        if oid != root {
            if parents.is_empty() || pending.len().saturating_add(parents.len()) > MAX_OPERATIONS {
                return Err(());
            }
            pending.extend(parents.iter().copied());
        }
        let author = commit.author();
        operations.insert(oid, json!({"id":oid.to_string(),"git_parents":commit.parent_ids().map(|id|id.to_string()).collect::<Vec<_>>(),"history_parents":parents.iter().map(ToString::to_string).collect::<Vec<_>>(),"message":commit.message().ok_or(())?,"author_name":author.name(),"author_email":author.email(),"recorded_at_ms":commit.time().seconds().checked_mul(1000),"signature_status":"not_verified","actions":contents}));
    }
    if !visited.contains(&root) || scan(&repository)?.get(&key) != Some(tips) {
        return Err(());
    }
    Ok(Some(
        json!({"identity":object,"operations":operations.into_values().collect::<Vec<_>>(),"operation_count":visited.len(),"retained_action_bytes":retained,"observation":"raw_local_history","signatures_verified":false,"reduced_state":null}),
    ))
}
