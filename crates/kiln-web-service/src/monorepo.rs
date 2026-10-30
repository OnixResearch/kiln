//! Imperative shell for monorepo facts. It reads exact Git objects only: a
//! bounded tree-to-tree diff between the exact base and candidate commits, and
//! bounded manifest blobs from the exact candidate tree. The affected decision
//! itself is the pure `kiln_core::monorepo::affected`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kiln_build_graph::ManifestTree;
use kiln_core::monorepo::{
    AffectedSet, ChangedFallback, ChangedFiles, ChangedSummary, GraphFacts, GraphKind, GraphResult,
    MAX_CHANGED_PATH_BYTES, MAX_CHANGED_PATHS, Outcome, ProjectInput, Reason,
};
use serde::{Deserialize, Serialize};

use crate::profile::ProjectProfile;

/// Named changed-file bounds; exceeding either yields `truncated`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChangedBounds {
    pub max_paths: usize,
    pub max_path_bytes: usize,
}

pub const CHANGED_BOUNDS: ChangedBounds = ChangedBounds {
    max_paths: MAX_CHANGED_PATHS,
    max_path_bytes: MAX_CHANGED_PATH_BYTES,
};

/// Why a run ran: the affected-set identity and this project's reasons.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAffected {
    pub set_identity: String,
    pub reasons: Vec<Reason>,
    pub changed: ChangedSummary,
}

/// Scope binding recorded on a scoped project's run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunScope {
    pub identity: String,
    pub root: String,
}

/// One evaluated repository event.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub set: AffectedSet,
    pub changed: ChangedSummary,
}

fn commit_tree<'r>(
    repository: &'r git2::Repository,
    revision: &str,
) -> Result<git2::Tree<'r>, git2::Error> {
    repository
        .find_commit(git2::Oid::from_str(revision)?)?
        .tree()
}

// r[impl kiln.monorepo.changed]
/// Exact changed paths between two commits from the object store. Both paths
/// of a rename (a delete plus an add, or a detected rename) are included.
/// No base, an over-bound diff and any read failure are explicit fallbacks.
pub fn changed_files(
    repository: &git2::Repository,
    base: Option<&str>,
    candidate: &str,
    bounds: ChangedBounds,
) -> ChangedFiles {
    let Some(base) = base else {
        return ChangedFiles::Fallback(ChangedFallback::NoBase);
    };
    let diff = (|| {
        let old = commit_tree(repository, base)?;
        let new = commit_tree(repository, candidate)?;
        let mut options = git2::DiffOptions::new();
        options.include_typechange(true);
        repository.diff_tree_to_tree(Some(&old), Some(&new), Some(&mut options))
    })();
    let Ok(diff) = diff else {
        return ChangedFiles::Fallback(ChangedFallback::ReadError);
    };
    let mut paths = BTreeSet::new();
    let mut bytes = 0usize;
    for delta in diff.deltas() {
        for file in [delta.old_file(), delta.new_file()] {
            let Some(path) = file.path_bytes() else {
                continue;
            };
            let Ok(path) = std::str::from_utf8(path) else {
                return ChangedFiles::Fallback(ChangedFallback::ReadError);
            };
            if paths.contains(path) {
                continue;
            }
            bytes = bytes.saturating_add(path.len());
            if paths.len() >= bounds.max_paths || bytes > bounds.max_path_bytes {
                return ChangedFiles::Fallback(ChangedFallback::Truncated);
            }
            paths.insert(path.to_owned());
        }
    }
    ChangedFiles::Exact(paths)
}

/// Bounded listing of the candidate tree plus the manifest blobs the graph
/// crate asks for. Over-bound or unreadable input is an unavailable reason.
pub fn manifest_tree(
    repository: &git2::Repository,
    candidate: &str,
) -> Result<ManifestTree, String> {
    let tree = commit_tree(repository, candidate).map_err(|_| "candidate_tree_unreadable")?;
    let mut paths = BTreeSet::new();
    let mut listing_bytes = 0usize;
    let mut over_bound = false;
    let mut invalid = false;
    tree.walk(git2::TreeWalkMode::PreOrder, |directory, entry| {
        if entry.kind() != Some(git2::ObjectType::Blob) {
            return git2::TreeWalkResult::Ok;
        }
        let Some(name) = entry.name() else {
            invalid = true;
            return git2::TreeWalkResult::Abort;
        };
        let path = format!("{directory}{name}");
        listing_bytes = listing_bytes.saturating_add(path.len());
        if paths.len() >= kiln_build_graph::MAX_TREE_PATHS
            || listing_bytes > kiln_build_graph::MAX_TREE_PATH_BYTES
        {
            over_bound = true;
            return git2::TreeWalkResult::Abort;
        }
        paths.insert(path);
        git2::TreeWalkResult::Ok
    })
    .or_else(|error| {
        if over_bound || invalid {
            Ok(())
        } else {
            Err(error)
        }
    })
    .map_err(|_| "candidate_tree_unreadable")?;
    if over_bound {
        return Err("candidate_tree_over_bound".into());
    }
    if invalid {
        return Err("candidate_tree_path_invalid".into());
    }
    let wanted = kiln_build_graph::wanted_files(&paths);
    if wanted.len() > kiln_build_graph::MAX_MANIFEST_FILES {
        return Err("manifest_files_over_bound".into());
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for path in wanted {
        let blob = tree
            .get_path(Path::new(&path))
            .and_then(|entry| entry.to_object(repository))
            .and_then(|object| object.peel_to_blob())
            .map_err(|_| "manifest_unreadable")?;
        total = total.saturating_add(blob.size());
        if blob.size() > kiln_build_graph::MAX_MANIFEST_FILE_BYTES
            || total > kiln_build_graph::MAX_MANIFEST_TOTAL_BYTES
        {
            return Err("manifest_over_bound".into());
        }
        files.insert(path, blob.content().to_vec());
    }
    Ok(ManifestTree { paths, files })
}

/// Graph facts for the requested kinds at the exact candidate.
pub fn graph_facts(
    repository: Option<&git2::Repository>,
    candidate: &str,
    kinds: &BTreeSet<GraphKind>,
) -> Vec<GraphFacts> {
    if kinds.is_empty() {
        return Vec::new();
    }
    let tree = match repository {
        Some(repository) => manifest_tree(repository, candidate),
        None => Err("repository_unreadable".into()),
    };
    kinds
        .iter()
        .map(|kind| match &tree {
            Ok(tree) => match kind {
                GraphKind::Cargo => kiln_build_graph::cargo_graph(tree),
                GraphKind::Nix => kiln_build_graph::nix_graph(tree),
            },
            Err(reason) => GraphFacts {
                kind: *kind,
                result: GraphResult::Unavailable(reason.clone()),
            },
        })
        .collect()
}

/// Inputs for every project in decision order.
pub fn inputs(projects: &[&ProjectProfile]) -> Result<Vec<ProjectInput>, String> {
    projects
        .iter()
        .map(|project| {
            Ok(ProjectInput {
                project_id: project.id.clone(),
                scope: project.admitted_scope()?,
            })
        })
        .collect()
}

// r[impl kiln.monorepo.dispatch]
/// One affected set for (repository, base, candidate, scope configuration).
/// Unreadable repositories never skip: facts fall back and every scoped
/// project is affected.
pub fn evaluate(
    repository_path: &Path,
    projects: &[&ProjectProfile],
    base: Option<&str>,
    candidate: &str,
) -> Result<Evaluation, String> {
    let inputs = inputs(projects)?;
    let kinds: BTreeSet<GraphKind> = inputs
        .iter()
        .filter_map(|input| input.scope.as_ref())
        .flat_map(|scope| scope.graphs().iter().copied())
        .collect();
    let repository = git2::Repository::open(repository_path).ok();
    let changed = match &repository {
        Some(repository) => changed_files(repository, base, candidate, CHANGED_BOUNDS),
        None => ChangedFiles::Fallback(ChangedFallback::ReadError),
    };
    let graphs = graph_facts(repository.as_ref(), candidate, &kinds);
    let set = kiln_core::monorepo::affected(&inputs, &changed, &graphs);
    Ok(Evaluation {
        changed: changed.summary(),
        set,
    })
}

impl Evaluation {
    /// Run record for an affected project, `None` when it was skipped.
    pub fn run_affected(&self, project_id: &str) -> Option<RunAffected> {
        match &self.set.decision(project_id)?.outcome {
            Outcome::Affected { reasons } => Some(RunAffected {
                set_identity: self.set.identity.as_str().to_owned(),
                reasons: reasons.clone(),
                changed: self.changed,
            }),
            Outcome::Skipped { .. } => None,
        }
    }
}

/// Manual and scheduled runs of a scoped project keep today's behaviour and
/// record the `no_base` fallback; unscoped projects record nothing.
pub fn manual_affected(project: &ProjectProfile) -> Result<Option<RunAffected>, String> {
    if project.scope.is_none() {
        return Ok(None);
    }
    let changed = ChangedFiles::Fallback(ChangedFallback::NoBase);
    let evaluation = Evaluation {
        set: kiln_core::monorepo::affected(&inputs(&[project])?, &changed, &[]),
        changed: changed.summary(),
    };
    Ok(evaluation.run_affected(&project.id))
}

/// Scope binding for a scoped project's run.
pub fn run_scope(project: &ProjectProfile) -> Result<Option<RunScope>, String> {
    Ok(project.admitted_scope()?.map(|admitted| RunScope {
        identity: admitted.identity().as_str().to_owned(),
        root: admitted.root().to_owned(),
    }))
}
