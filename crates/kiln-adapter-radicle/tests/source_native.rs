#![cfg(feature = "source-native-fixtures")]

use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use cob_git2 as git2;
use git2::{Oid, Repository};
use kiln_adapter_radicle::source::{RadicleSourceProfile, RadicleSourceShell};
use kiln_provider_api::{PortFailure, SourceRepositoryPort};
use kiln_radicle_source_contract::{
    CONTRACT_SCHEMA_VERSION, FeatureLevel, SnapshotLimits, SourceRequest,
};
use radicle_core::RepoId;
use radicle_crypto::signature::{Keypair as _, Signer as _};
use radicle_crypto::{Seed, SigningKey};
use tempfile::TempDir;

const MODE_TREE: i32 = 0o040_000;
const MODE_REGULAR: i32 = 0o100_644;
const SIGNATURE_BYTES: usize = 64;
const SIGNER_SEED_BYTES: usize = 32;
const SHA1_BYTES: usize = 20;
const SIGNER_SEED_VALUE: u8 = 7;
const MAX_IDENTITY_BYTES: u64 = 256;
const MAX_REFERENCE_BYTES: u64 = 256;
const MAX_ENTRIES: u64 = 128;
const MAX_PATH_BYTES: u64 = 512;
const MAX_FILE_BYTES: u64 = 4096;
const MAX_TOTAL_BYTES: u64 = 16384;
const SOURCE_CONTENT: &[u8] = b"bounded Radicle source\n";
const IDENTITY_CONTENT: &[u8] = b"{\"payload\":\"fixture\"}\n";
const SOURCE_REFERENCE: &str = "refs/heads/main";
const IDENTITY_ROOT_REFERENCE: &str = "refs/rad/root";
const SIGREFS_PARENT_REFERENCE: &str = "refs/rad/sigrefs-parent";

struct Fixture {
    _directory: TempDir,
    repository_path: PathBuf,
    profile: RadicleSourceProfile,
    signer: SigningKey,
    identity_root: Oid,
    source_revision: Oid,
    sigrefs_commit: Oid,
}

fn port<T>(result: Result<T, PortFailure>) -> Result<T, io::Error> {
    result.map_err(|_| io::Error::other("Radicle source fixture operation failed"))
}

fn commit_tree(
    repository: &Repository,
    tree: Oid,
    parents: &[Oid],
    message: &str,
) -> Result<Oid, Box<dyn Error>> {
    let tree = repository.find_tree(tree)?;
    let parents = parents
        .iter()
        .map(|oid| repository.find_commit(*oid))
        .collect::<Result<Vec<_>, _>>()?;
    let parent_refs = parents.iter().collect::<Vec<_>>();
    let signature = git2::Signature::now("Kiln fixture", "kiln@invalid.example")?;
    Ok(repository.commit(None, &signature, &signature, message, &tree, &parent_refs)?)
}

fn identity_commit(repository: &Repository) -> Result<(Oid, RepoId), Box<dyn Error>> {
    let document = repository.blob(IDENTITY_CONTENT)?;
    let mut embeds = repository.treebuilder(None)?;
    embeds.insert("radicle.json", document, MODE_REGULAR)?;
    let embeds = embeds.write()?;
    let mut root = repository.treebuilder(None)?;
    root.insert("embeds", embeds, MODE_TREE)?;
    let root = root.write()?;
    let commit = commit_tree(repository, root, &[], "fixture identity")?;
    Ok((commit, RepoId::from(document)))
}

fn source_commit(repository: &Repository, content: &[u8]) -> Result<Oid, Box<dyn Error>> {
    let blob = repository.blob(content)?;
    let mut tree = repository.treebuilder(None)?;
    tree.insert("source.txt", blob, MODE_REGULAR)?;
    let tree = tree.write()?;
    commit_tree(repository, tree, &[], "fixture source")
}

fn canonical_refs(refs: &BTreeMap<String, Oid>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, oid) in refs {
        bytes.extend_from_slice(oid.to_string().as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

fn sigrefs_commit(
    repository: &Repository,
    signer: &SigningKey,
    identity_root: Oid,
    source: Oid,
    parent: Option<Oid>,
    valid_signature: bool,
) -> Result<Oid, Box<dyn Error>> {
    let mut refs = BTreeMap::from([
        (IDENTITY_ROOT_REFERENCE.to_string(), identity_root),
        (SOURCE_REFERENCE.to_string(), source),
    ]);
    if let Some(parent) = parent {
        refs.insert(SIGREFS_PARENT_REFERENCE.to_string(), parent);
    }
    let refs = canonical_refs(&refs);
    let signature = if valid_signature {
        signer.try_sign(&refs)?.to_vec()
    } else {
        vec![0; SIGNATURE_BYTES]
    };
    let refs_blob = repository.blob(&refs)?;
    let signature_blob = repository.blob(&signature)?;
    let mut tree = repository.treebuilder(None)?;
    tree.insert("refs", refs_blob, MODE_REGULAR)?;
    tree.insert("signature", signature_blob, MODE_REGULAR)?;
    let tree = tree.write()?;
    let parents = parent.map_or_else(Vec::new, |oid| vec![oid]);
    commit_tree(repository, tree, &parents, "fixture signed refs")
}

fn request(rid: RepoId, signer: &SigningKey, sigrefs: Oid, revision: Oid) -> SourceRequest {
    SourceRequest {
        schema_version: CONTRACT_SCHEMA_VERSION,
        profile_id: "radicle-source-v1".to_string(),
        repository_id: rid.to_string(),
        signer_node_id: signer.verifying_key().public_key().to_string(),
        signed_reference: SOURCE_REFERENCE.to_string(),
        expected_sigrefs_commit: sigrefs.to_string(),
        expected_revision: revision.to_string(),
        limits: SnapshotLimits {
            max_identity_bytes: MAX_IDENTITY_BYTES,
            max_reference_bytes: MAX_REFERENCE_BYTES,
            max_entries: MAX_ENTRIES,
            max_path_bytes: MAX_PATH_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
        },
    }
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let repository_path = directory.path().join("repository.git");
    let repository = Repository::init_bare(&repository_path)?;
    let (identity_root, rid) = identity_commit(&repository)?;
    let source_revision = source_commit(&repository, SOURCE_CONTENT)?;
    let signer = SigningKey::from_seed(Seed::new([SIGNER_SEED_VALUE; SIGNER_SEED_BYTES]));
    let first = sigrefs_commit(
        &repository,
        &signer,
        identity_root,
        source_revision,
        None,
        true,
    )?;
    let sigrefs_commit = sigrefs_commit(
        &repository,
        &signer,
        identity_root,
        source_revision,
        Some(first),
        true,
    )?;
    let node = signer.verifying_key().public_key().to_string();
    repository.reference(
        &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
        sigrefs_commit,
        true,
        "fixture sigrefs",
    )?;
    repository.reference(
        &format!("refs/namespaces/{node}/{SOURCE_REFERENCE}"),
        source_revision,
        true,
        "fixture source",
    )?;
    let profile = RadicleSourceProfile::pinned(
        repository_path.clone(),
        request(rid, &signer, sigrefs_commit, source_revision),
    );
    Ok(Fixture {
        _directory: directory,
        repository_path,
        profile,
        signer,
        identity_root,
        source_revision,
        sigrefs_commit,
    })
}

// r[verify kiln.radicle.shell_source.scenario.stable]
// r[verify kiln.radicle.shell_conformance]
#[test]
fn native_parent_signed_source_resolves_repeatably() -> Result<(), Box<dyn Error>> {
    let fixture = fixture()?;
    let shell = port(RadicleSourceShell::new(fixture.profile))?;
    let first = port(shell.resolve_and_acquire())?;
    let second = port(shell.resolve_and_acquire())?;

    assert_eq!(first, second);
    assert_eq!(
        first.receipt.observation.feature_level,
        FeatureLevel::Parent
    );
    assert_eq!(
        first.resolution.exact_revision.native_id.as_str(),
        fixture.source_revision.to_string()
    );
    let symbolic = first.resolution.symbolic_reference.clone();
    let resolved = port(shell.resolve(&symbolic))?;
    assert_eq!(resolved.exact_revisions.len(), 1);
    let manifest = port(shell.acquire_canonical_manifest(&first.resolution.exact_revision))?;
    assert!(!manifest.is_empty());
    Ok(())
}

// r[verify kiln.radicle.shell_source.scenario.moved]
// r[verify kiln.radicle.shell_conformance]
#[test]
fn stale_sigrefs_and_namespace_substitution_fail_closed() -> Result<(), Box<dyn Error>> {
    let fixture = fixture()?;
    let repository = Repository::open(&fixture.repository_path)?;
    let newer_source = source_commit(&repository, b"substituted source\n")?;
    let node = fixture.signer.verifying_key().public_key().to_string();
    repository.reference(
        &format!("refs/namespaces/{node}/{SOURCE_REFERENCE}"),
        newer_source,
        true,
        "move source",
    )?;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    assert!(shell.resolve_and_acquire().is_err());

    repository.reference(
        &format!("refs/namespaces/{node}/{SOURCE_REFERENCE}"),
        fixture.source_revision,
        true,
        "restore source",
    )?;
    let newer_sigrefs = sigrefs_commit(
        &repository,
        &fixture.signer,
        fixture.identity_root,
        fixture.source_revision,
        Some(fixture.sigrefs_commit),
        true,
    )?;
    repository.reference(
        &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
        newer_sigrefs,
        true,
        "move sigrefs",
    )?;
    assert!(shell.resolve_and_acquire().is_err());
    Ok(())
}

// r[verify kiln.radicle.shell_conformance]
#[test]
fn invalid_signature_identity_and_bounds_fail_closed() -> Result<(), Box<dyn Error>> {
    let fixture = fixture()?;
    let repository = Repository::open(&fixture.repository_path)?;
    let invalid = sigrefs_commit(
        &repository,
        &fixture.signer,
        fixture.identity_root,
        fixture.source_revision,
        Some(fixture.sigrefs_commit),
        false,
    )?;
    let node = fixture.signer.verifying_key().public_key().to_string();
    repository.reference(
        &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
        invalid,
        true,
        "invalid signature",
    )?;
    let mut invalid_signature = fixture.profile.clone();
    invalid_signature.request.expected_sigrefs_commit = invalid.to_string();
    let shell = port(RadicleSourceShell::new(invalid_signature))?;
    assert!(shell.resolve_and_acquire().is_err());

    let mut wrong_rid = fixture.profile.clone();
    wrong_rid.request.repository_id = RepoId::from(Oid::from_bytes(&[1; SHA1_BYTES])?).to_string();
    let shell = port(RadicleSourceShell::new(wrong_rid))?;
    assert!(shell.resolve_and_acquire().is_err());

    let mut bounded = fixture.profile;
    bounded.request.limits.max_file_bytes = 1;
    let shell = port(RadicleSourceShell::new(bounded))?;
    assert!(shell.resolve_and_acquire().is_err());
    Ok(())
}
