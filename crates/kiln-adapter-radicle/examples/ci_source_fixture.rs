//! Isolated signed-source producer for the maintained native CI exercise only.
//! Never opens an operator repository. Native Radicle initializes the isolated
//! identity; a new private fixture marker bounds subsequent source revisions.
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use cob_git2::{Oid, Repository};
use kiln_adapter_radicle::source::RadicleSourceSelection;
use kiln_radicle_source_contract::SnapshotLimits;
use radicle_core::RepoId;
use radicle_crypto::SigningKey;
use radicle_crypto::signature::{Keypair as _, Signer as _};

const FILE_MODE: i32 = 0o100_644;
const TREE_MODE: i32 = 0o040_000;
const PROJECT_NAME: &str = "kiln-source-ci-isolated-fixture";
const MARKER: &str = "kiln.radicle-ci.isolated-source-fixture.v1";
const MAX_ENTRIES: usize = 512;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

fn commit(
    repo: &Repository,
    tree: Oid,
    parents: &[Oid],
    message: &str,
) -> Result<Oid, Box<dyn Error>> {
    let tree = repo.find_tree(tree)?;
    let parents = parents
        .iter()
        .map(|oid| repo.find_commit(*oid))
        .collect::<Result<Vec<_>, _>>()?;
    let parents = parents.iter().collect::<Vec<_>>();
    let author = cob_git2::Signature::now("Kiln isolated fixture", "kiln@invalid.example")?;
    Ok(repo.commit(None, &author, &author, message, &tree, &parents)?)
}
fn source_tree(
    repo: &Repository,
    path: &Path,
    budget: &mut (usize, usize),
    depth: usize,
) -> Result<Oid, Box<dyn Error>> {
    if depth > MAX_DEPTH {
        return Err("fixture tree depth bound exceeded".into());
    }
    let mut tree = repo.treebuilder(None)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        budget.0 += 1;
        if budget.0 > MAX_ENTRIES {
            return Err("fixture entry bound exceeded".into());
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        let (oid, mode) = if metadata.is_dir() {
            (
                source_tree(repo, &entry.path(), budget, depth + 1)?,
                TREE_MODE,
            )
        } else if metadata.is_file() {
            let mut bytes = Vec::new();
            fs::File::open(entry.path())?
                .take((MAX_FILE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            budget.1 += bytes.len();
            if bytes.len() > MAX_FILE_BYTES || budget.1 > MAX_TOTAL_BYTES {
                return Err("fixture byte bound exceeded".into());
            }
            (
                repo.blob(&bytes)?,
                if metadata.permissions().mode() & 0o111 != 0 {
                    0o100_755
                } else {
                    FILE_MODE
                },
            )
        } else {
            return Err("fixture accepts only regular files/directories".into());
        };
        tree.insert(entry.file_name(), oid, mode)?;
    }
    Ok(tree.write()?)
}
fn signed_refs(
    repo: &Repository,
    signer: &SigningKey,
    refs: BTreeMap<String, Oid>,
    parent: Option<Oid>,
) -> Result<Oid, Box<dyn Error>> {
    let mut bytes = Vec::new();
    for (name, oid) in refs {
        bytes.extend_from_slice(format!("{oid} {name}\n").as_bytes());
    }
    let signature = signer.try_sign(&bytes)?;
    let mut tree = repo.treebuilder(None)?;
    tree.insert("refs", repo.blob(&bytes)?, FILE_MODE)?;
    tree.insert("signature", repo.blob(&signature.to_bytes())?, FILE_MODE)?;
    commit(
        repo,
        tree.write()?,
        &parent.into_iter().collect::<Vec<_>>(),
        "fixture signed refs",
    )
}
fn initialize(root: &Path, source: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    fs::write(root.join("fixture-owner.json"), MARKER)?;
    let worktree = root.join("worktree");
    let repo = Repository::init(&worktree)?;
    let tree = source_tree(&repo, source, &mut (0, 0), 0)?;
    let revision = commit(&repo, tree, &[], "isolated source bootstrap")?;
    repo.reference("refs/heads/main", revision, true, "isolated bootstrap")?;
    repo.set_head("refs/heads/main")?;
    repo.checkout_head(None)?;
    println!(
        "{}",
        serde_json::json!({"worktree": worktree, "project_name": PROJECT_NAME})
    );
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if let [mode, root, source] = arguments.as_slice()
        && mode == "init"
    {
        return initialize(Path::new(root), Path::new(source));
    }
    let [mode, root, source, home, rid] = arguments.as_slice() else {
        return Err("usage: ci_source_fixture init NEW_PRIVATE_ROOT SOURCE | advance OWNED_ROOT SOURCE OWNED_RAD_HOME RID".into());
    };
    let root = Path::new(root);
    let source = Path::new(source);
    let home = Path::new(home);
    if mode != "advance"
        || fs::read_to_string(root.join("fixture-owner.json"))? != MARKER
        || root
            .parent()
            .map(|parent| parent.join("rad-home"))
            .as_deref()
            != Some(home)
    {
        return Err("only this newly initialized isolated fixture may be advanced".into());
    }
    let rid: RepoId = rid.parse()?;
    let repo = Repository::open(
        home.join("storage")
            .join(rid.to_string().trim_start_matches("rad:")),
    )?;
    let signer = radicle_crypto::ssh::Keystore::new(&home.join("keys"))
        .secret_key(None)?
        .ok_or("isolated profile key is missing")?;
    let node = signer.verifying_key().public_key().to_string();
    let branch = format!("refs/namespaces/{node}/refs/heads/main");
    let sigref = format!("refs/namespaces/{node}/refs/rad/sigrefs");
    let parent = repo
        .find_reference(&sigref)?
        .target()
        .ok_or("isolated signed refs are not direct")?;
    let previous = repo
        .find_reference(&branch)?
        .target()
        .ok_or("isolated source is not direct")?;
    let identity = repo.find_reference("refs/rad/id")?.peel_to_commit()?.id();
    let source_tree = source_tree(&repo, source, &mut (0, 0), 0)?;
    let revision = commit(
        &repo,
        source_tree,
        &[previous],
        "fixture exact project source",
    )?;
    let mut refs = BTreeMap::from([
        ("refs/heads/main".into(), revision),
        ("refs/rad/root".into(), identity),
    ]);
    refs.insert("refs/rad/sigrefs-parent".into(), parent);
    let signed = signed_refs(&repo, &signer, refs, Some(parent))?;
    repo.reference(&branch, revision, true, "isolated fixture source")?;
    repo.reference(&sigref, signed, true, "isolated fixture sigrefs")?;
    repo.reference(
        "refs/heads/main",
        revision,
        true,
        "isolated fixture producer lookup",
    )?;
    repo.set_head("refs/heads/main")?;
    let selection = RadicleSourceSelection {
        repository_path: repo.path().into(),
        repository_id: rid.to_string(),
        signer_node_id: node,
        signed_reference: "refs/heads/main".into(),
        profile_id: "radicle-source-v1".into(),
        limits: SnapshotLimits {
            max_identity_bytes: 4096,
            max_reference_bytes: 512,
            max_entries: MAX_ENTRIES as u64,
            max_path_bytes: 1024,
            max_file_bytes: MAX_FILE_BYTES as u64,
            max_total_bytes: MAX_TOTAL_BYTES as u64,
        },
    };
    selection.admit().map_err(|error| format!("{error:?}"))?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"project_name": PROJECT_NAME, "selection":selection,"source":{"provider":"radicle","repository":rid.to_string(),"revision":revision.to_string()}})
        )?
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("isolated fixture: {error}");
        std::process::exit(1);
    }
}
