#![cfg(feature = "source-native-fixtures")]

use std::collections::BTreeMap;
use std::error::Error;
use std::io;

use cob_git2 as git2;
use git2::{Oid, Repository};
use heartwood_canonical::identity::Did;
use kiln_adapter_radicle::source::{RadicleSourceProfile, RadicleSourceShell};
use kiln_provider_api::PortFailure;
use kiln_radicle_source_contract::{
    CONTRACT_SCHEMA_VERSION, CanonicalRequest, SnapshotLimits, SourceRequest,
};
use radicle_core::RepoId;
use radicle_crypto::signature::{Keypair as _, Signer as _};
use radicle_crypto::{PublicKey, Seed, SigningKey};
use tempfile::TempDir;

const MODE_TREE: i32 = 0o040_000;
const MODE_REGULAR: i32 = 0o100_644;
const SIGNER_SEED_BYTES: usize = 32;
const MAX_IDENTITY_BYTES: u64 = 4096;
const MAX_REFERENCE_BYTES: u64 = 512;
const MAX_ENTRIES: u64 = 1024;
const MAX_PATH_BYTES: u64 = 1024;
const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_TOTAL_BYTES: u64 = 16_777_216;
const SOURCE_REFERENCE: &str = "refs/heads/main";
const TAG_REFERENCE: &str = "refs/tags/release/v1";
const IDENTITY_ROOT_REFERENCE: &str = "refs/rad/root";

fn port<T>(result: Result<T, PortFailure>) -> Result<T, io::Error> {
    result.map_err(|_| io::Error::other("Radicle canonical fixture operation failed"))
}

fn code(result: Result<impl Sized, PortFailure>) -> String {
    match result {
        Err(PortFailure::Domain(diagnostic)) => diagnostic.code.to_string(),
        Err(_) => "non_domain".to_string(),
        Ok(_) => "accepted".to_string(),
    }
}

fn signer(seed: u8) -> SigningKey {
    SigningKey::from_seed(Seed::new([seed; SIGNER_SEED_BYTES]))
}

fn did(key: &PublicKey) -> String {
    Did::from(key).encode()
}

fn identity_document(delegates: &[PublicKey], threshold: usize, reference: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "payload": {
            "xyz.radicle.project": {"name": "canonical-fixture"},
            "xyz.radicle.crefs": {
                reference: {"allow": "delegates", "threshold": threshold}
            }
        },
        "delegates": delegates.iter().map(did).collect::<Vec<_>>(),
        "threshold": threshold,
        "visibility": "public"
    }))
    .unwrap_or_else(|error| unreachable!("fixture document: {error:?}"))
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

fn identity_root(
    repository: &Repository,
    document: &[u8],
) -> Result<(Oid, RepoId), Box<dyn Error>> {
    let document = repository.blob(document)?;
    let mut embeds = repository.treebuilder(None)?;
    embeds.insert("radicle.json", document, MODE_REGULAR)?;
    let embeds = embeds.write()?;
    let mut root = repository.treebuilder(None)?;
    root.insert("embeds", embeds, MODE_TREE)?;
    let root = root.write()?;
    let commit = commit_tree(repository, root, &[], "fixture identity root")?;
    Ok((commit, RepoId::from(document)))
}

fn source_commit(repository: &Repository, content: &[u8]) -> Result<Oid, Box<dyn Error>> {
    let blob = repository.blob(content)?;
    let mut tree = repository.treebuilder(None)?;
    tree.insert("source.txt", blob, MODE_REGULAR)?;
    let tree = tree.write()?;
    commit_tree(repository, tree, &[], "fixture source")
}

fn signed_refs(refs: &BTreeMap<String, Oid>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, oid) in refs {
        bytes.extend_from_slice(oid.to_string().as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// One delegate's signed refs commit, embedding the identity document.
fn sigrefs_commit(
    repository: &Repository,
    signer: &SigningKey,
    document: &[u8],
    identity_root: Oid,
    reference: &str,
    source: Oid,
) -> Result<Oid, Box<dyn Error>> {
    let refs = signed_refs(&BTreeMap::from([
        (IDENTITY_ROOT_REFERENCE.to_string(), identity_root),
        (reference.to_string(), source),
    ]));
    let signature = signer.try_sign(&refs)?.to_vec();
    let refs_blob = repository.blob(&refs)?;
    let signature_blob = repository.blob(&signature)?;
    let document_blob = repository.blob(document)?;
    let mut embeds = repository.treebuilder(None)?;
    embeds.insert("radicle.json", document_blob, MODE_REGULAR)?;
    let embeds = embeds.write()?;
    let mut tree = repository.treebuilder(None)?;
    tree.insert("refs", refs_blob, MODE_REGULAR)?;
    tree.insert("signature", signature_blob, MODE_REGULAR)?;
    tree.insert("embeds", embeds, MODE_TREE)?;
    let tree = tree.write()?;
    commit_tree(repository, tree, &[], "fixture signed refs")
}

struct Fixture {
    _directory: TempDir,
    profile: RadicleSourceProfile,
    object: Oid,
    tag_object: Option<Oid>,
    identity: Oid,
}

/// Build a repository where each delegate in `votes` publishes the reference.
/// `alternate` is the second tip used by the divergence fixture.
fn fixture_with_reference(
    reference: &str,
    annotated_tag: bool,
    threshold: usize,
    delegates: &[u8],
    votes: &[(u8, bool)],
    alternate: Option<&[u8]>,
) -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let repository_path = directory.path().join("repository.git");
    let repository = Repository::init_bare(&repository_path)?;
    let keys: Vec<SigningKey> = delegates.iter().map(|seed| signer(*seed)).collect();
    let publics: Vec<PublicKey> = keys
        .iter()
        .map(|key| *key.verifying_key().public_key())
        .collect();
    let document = identity_document(&publics, threshold, reference);
    let (identity, rid) = identity_root(&repository, &document)?;
    let object = source_commit(&repository, b"canonical source\n")?;
    let tag_object = if annotated_tag {
        let signature = git2::Signature::now("Kiln fixture", "kiln@invalid.example")?;
        let target = repository.find_object(object, None)?;
        Some(repository.tag("release/v1", &target, &signature, "fixture release", false)?)
    } else {
        None
    };
    let source_target = tag_object.unwrap_or(object);
    let alternate = alternate
        .map(|content| source_commit(&repository, content))
        .transpose()?;
    let mut primary = None;
    for (seed, use_alternate) in votes {
        let key = signer(*seed);
        let node = key.verifying_key().public_key().to_string();
        let voted = if *use_alternate {
            alternate.ok_or("alternate vote without an alternate tip")?
        } else {
            source_target
        };
        let sigrefs = sigrefs_commit(&repository, &key, &document, identity, reference, voted)?;
        repository.reference(
            &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
            sigrefs,
            true,
            "fixture sigrefs",
        )?;
        repository.reference(
            &format!("refs/namespaces/{node}/{reference}"),
            voted,
            true,
            "fixture source",
        )?;
        if delegates.first() == Some(seed) {
            primary = Some((node, sigrefs));
        }
    }
    let (node, sigrefs) = primary.ok_or("the first delegate must vote in every fixture")?;
    let request = SourceRequest {
        schema_version: CONTRACT_SCHEMA_VERSION,
        profile_id: "radicle-source-v1".to_string(),
        repository_id: rid.to_string(),
        signer_node_id: node,
        signed_reference: reference.to_string(),
        expected_sigrefs_commit: sigrefs.to_string(),
        expected_revision: object.to_string(),
        limits: SnapshotLimits {
            max_identity_bytes: MAX_IDENTITY_BYTES,
            max_reference_bytes: MAX_REFERENCE_BYTES,
            max_entries: MAX_ENTRIES,
            max_path_bytes: MAX_PATH_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
        },
        canonical: Some(CanonicalRequest {
            identity_commit: identity.to_string(),
            reference: reference.to_string(),
        }),
    };
    let profile = RadicleSourceProfile::pinned(repository_path.clone(), request);
    Ok(Fixture {
        _directory: directory,
        profile,
        object,
        tag_object,
        identity,
    })
}

fn fixture(
    threshold: usize,
    delegates: &[u8],
    votes: &[(u8, bool)],
    alternate: Option<&[u8]>,
) -> Result<Fixture, Box<dyn Error>> {
    fixture_with_reference(SOURCE_REFERENCE, false, threshold, delegates, votes, alternate)
}

// r[verify kiln.radicle.canonical_source] r[verify kiln.radicle.canonical_source.scenario.quorum]
#[test]
fn two_of_three_delegates_accept_the_canonical_head() -> Result<(), Box<dyn Error>> {
    let fixture = fixture(2, &[11, 12, 13], &[(11, false), (12, false)], None)?;
    let object = fixture.object;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    let result = port(shell.resolve_and_acquire())?;
    let evidence = result
        .receipt
        .canonical
        .as_ref()
        .ok_or("canonical evidence")?;
    assert_eq!(evidence.threshold, 2);
    assert_eq!(evidence.delegates.len(), 3);
    assert_eq!(evidence.identity_commit, fixture.identity.to_string());
    assert_eq!(evidence.identity_root, fixture.identity.to_string());
    assert_eq!(evidence.object, object.to_string());
    assert_eq!(evidence.reference, SOURCE_REFERENCE);
    assert_eq!(evidence.object_kind, "commit");
    assert_eq!(evidence.observations.len(), 2);
    assert!(
        evidence
            .observations
            .iter()
            .all(|observed| observed.object == object.to_string())
    );
    assert_eq!(
        result.resolution.exact_revision.native_id.as_str(),
        object.to_string()
    );
    Ok(())
}

// r[verify kiln.radicle.canonical_tag] r[verify kiln.radicle.canonical_tag.scenario.tag]
#[test]
fn annotated_tag_source_peels_to_its_commit_and_retains_tag_evidence() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_with_reference(TAG_REFERENCE, true, 1, &[51], &[(51, false)], None)?;
    let tag_object = fixture.tag_object.ok_or("annotated tag object")?;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    let result = shell
        .resolve_and_acquire()
        .map_err(|error| io::Error::other(format!("tag fixture: {error:?}")))?;
    let evidence = result.receipt.canonical.as_ref().ok_or("canonical evidence")?;
    assert_eq!(evidence.reference, TAG_REFERENCE);
    assert_eq!(evidence.object, tag_object.to_string());
    assert_eq!(evidence.object_kind, "tag");
    assert_eq!(evidence.revision, fixture.object.to_string());
    assert_eq!(result.receipt.snapshot.revision, fixture.object.to_string());
    assert_eq!(
        result.resolution.exact_revision.native_id.as_str(),
        fixture.object.to_string()
    );
    Ok(())
}

// r[verify kiln.radicle.canonical_tag] r[verify kiln.radicle.canonical_tag.scenario.tag]
#[test]
fn lightweight_tag_source_uses_its_commit_directly() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_with_reference(TAG_REFERENCE, false, 1, &[71], &[(71, false)], None)?;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    let result = port(shell.resolve_and_acquire())?;
    let evidence = result.receipt.canonical.as_ref().ok_or("canonical evidence")?;
    assert_eq!(evidence.reference, TAG_REFERENCE);
    assert_eq!(evidence.object, fixture.object.to_string());
    assert_eq!(evidence.object_kind, "commit");
    assert_eq!(evidence.revision, fixture.object.to_string());
    assert_eq!(result.receipt.snapshot.revision, fixture.object.to_string());
    Ok(())
}

// r[verify kiln.radicle.canonical_tag.scenario.revision]
#[test]
fn tag_target_that_is_not_a_commit_fails_before_snapshot() -> Result<(), Box<dyn Error>> {
    let mut fixture = fixture_with_reference(TAG_REFERENCE, true, 1, &[61], &[(61, false)], None)?;
    let repository = Repository::open(&fixture.profile.repository_path)?;
    let key = signer(61);
    let node = key.verifying_key().public_key().to_string();
    let document = identity_document(
        &[*key.verifying_key().public_key()],
        1,
        TAG_REFERENCE,
    );
    let blob = repository.blob(b"not a commit")?;
    let target = repository.find_object(blob, None)?;
    let signature = git2::Signature::now("Kiln fixture", "kiln@invalid.example")?;
    let tag = repository.tag("bad-release/v1", &target, &signature, "bad tag", false)?;
    let sigrefs = sigrefs_commit(
        &repository,
        &key,
        &document,
        fixture.identity,
        TAG_REFERENCE,
        tag,
    )?;
    repository.reference(
        &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
        sigrefs,
        true,
        "bad tag sigrefs",
    )?;
    repository.reference(
        &format!("refs/namespaces/{node}/{TAG_REFERENCE}"),
        tag,
        true,
        "bad tag source",
    )?;
    fixture.profile.request.expected_sigrefs_commit = sigrefs.to_string();
    let shell = port(RadicleSourceShell::new(fixture.profile))?;
    match shell.resolve_and_acquire() {
        Err(PortFailure::Infrastructure(error)) => {
            assert_eq!(error.code, "radicle_source_git");
        }
        other => return Err(format!("expected a non-commit tag failure, got {other:?}").into()),
    }
    Ok(())
}

// r[verify kiln.radicle.canonical_source.scenario.insufficient]
#[test]
fn a_single_vote_never_reaches_the_threshold() -> Result<(), Box<dyn Error>> {
    let fixture = fixture(2, &[21, 22, 23], &[(21, false)], None)?;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    assert_eq!(
        code(shell.resolve_and_acquire().map(|_| ())),
        "radicle_source_canonical_quorum"
    );
    Ok(())
}

// r[verify kiln.radicle.canonical_source.scenario.diverging]
#[test]
fn diverging_delegate_tips_have_no_canonical_head() -> Result<(), Box<dyn Error>> {
    // Two delegates vote for each tip under a 2-of-4 threshold.
    let fixture = fixture(
        2,
        &[31, 32, 33, 34],
        &[(31, false), (32, false), (33, true), (34, true)],
        Some(b"diverging second\n"),
    )?;
    let shell = port(RadicleSourceShell::new(fixture.profile.clone()))?;
    let observed = code(shell.resolve_and_acquire().map(|_| ()));
    assert!(
        observed == "radicle_source_canonical_quorum"
            || observed == "radicle_source_canonical_objects",
        "unexpected canonical failure: {observed}"
    );
    Ok(())
}

// r[verify kiln.radicle.canonical_source.identity.scenario.revision]
#[test]
fn a_delegate_with_a_different_identity_revision_is_refused() -> Result<(), Box<dyn Error>> {
    let fixture = fixture(2, &[41, 42], &[(41, false), (42, false)], None)?;
    let repository = Repository::open(&fixture.profile.repository_path)?;
    let other_document = identity_document(
        &[
            *signer(41).verifying_key().public_key(),
            *signer(42).verifying_key().public_key(),
        ],
        1,
        SOURCE_REFERENCE,
    );
    let key = signer(42);
    let node = key.verifying_key().public_key().to_string();
    let sigrefs = sigrefs_commit(
        &repository,
        &key,
        &other_document,
        fixture.identity,
        SOURCE_REFERENCE,
        fixture.object,
    )?;
    repository.reference(
        &format!("refs/namespaces/{node}/refs/rad/sigrefs"),
        sigrefs,
        true,
        "different document sigrefs",
    )?;
    let shell = port(RadicleSourceShell::new(fixture.profile))?;
    assert_eq!(
        code(shell.resolve_and_acquire().map(|_| ())),
        "radicle_source_canonical_identity_revision"
    );
    Ok(())
}
