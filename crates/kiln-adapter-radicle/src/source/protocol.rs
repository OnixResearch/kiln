use std::collections::BTreeMap;
use std::path::Path;

use cob_git2 as git2;
use git2::{ObjectType, Oid, Repository};
use kiln_provider_api::PortResult;
use radicle_core::RepoId;
use radicle_crypto::signature::Verifier as _;
use radicle_crypto::{PublicKey, Signature as RadicleSignature, VerifyingKey};
use radicle_git_ref_format::RefString;

use super::error::{domain, git_failure};
use super::profile::RadicleSourceProfile;

const REFS_BLOB_PATH: &str = "refs";
const SIGNATURE_BLOB_PATH: &str = "signature";
const IDENTITY_DOCUMENT_PATH: &str = "embeds/radicle.json";
const IDENTITY_ROOT_REFERENCE: &str = "refs/rad/root";
const SIGREFS_PARENT_REFERENCE: &str = "refs/rad/sigrefs-parent";
const SIGREFS_REFERENCE: &str = "refs/rad/sigrefs";

#[derive(Debug)]
pub(super) struct VerifiedSignedRefs {
    pub sigrefs_commit: Oid,
    pub sigrefs_parent: Option<Oid>,
    pub identity_root_commit: Oid,
    pub signed_revision: Oid,
    pub namespaced_reference: String,
}

fn sigrefs_reference(profile: &RadicleSourceProfile) -> String {
    format!(
        "refs/namespaces/{}/{SIGREFS_REFERENCE}",
        profile.request.signer_node_id
    )
}

fn namespaced_source_reference(profile: &RadicleSourceProfile) -> String {
    format!(
        "refs/namespaces/{}/{}",
        profile.request.signer_node_id, profile.request.signed_reference
    )
}

pub(super) fn reference_target(repository: &Repository, reference: &str) -> PortResult<Oid> {
    repository
        .find_reference(reference)
        .map_err(git_failure)?
        .target()
        .ok_or_else(|| {
            domain(
                "radicle_source_symbolic_reference",
                "radicle_source.reference",
                "Radicle source reference is not a direct object reference",
            )
        })
}

fn read_blob(
    repository: &Repository,
    commit: &git2::Commit<'_>,
    path: &str,
) -> PortResult<Vec<u8>> {
    let tree = commit.tree().map_err(git_failure)?;
    let entry = tree.get_path(Path::new(path)).map_err(git_failure)?;
    if entry.kind() != Some(ObjectType::Blob) {
        return Err(domain(
            "radicle_source_blob_kind",
            "radicle_source.sigrefs",
            "Radicle signed-reference entry is not a blob",
        ));
    }
    let blob = repository.find_blob(entry.id()).map_err(git_failure)?;
    Ok(blob.content().to_vec())
}

fn parse_refs(
    profile: &RadicleSourceProfile,
    bytes: &[u8],
) -> PortResult<(BTreeMap<String, Oid>, Vec<u8>)> {
    let byte_count = u64::try_from(bytes.len()).map_err(|_| {
        domain(
            "radicle_source_sigrefs_bound",
            "radicle_source.sigrefs",
            "Radicle signed-reference bytes exceed the supported range",
        )
    })?;
    if byte_count > profile.request.limits.max_total_bytes {
        return Err(domain(
            "radicle_source_sigrefs_bound",
            "radicle_source.sigrefs",
            "Radicle signed-reference bytes exceed the request bound",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        domain(
            "radicle_source_sigrefs_encoding",
            "radicle_source.sigrefs",
            "Radicle signed-reference bytes are not valid UTF-8",
        )
    })?;
    let maximum_entries = usize::try_from(profile.request.limits.max_entries).map_err(|_| {
        domain(
            "radicle_source_sigrefs_bound",
            "radicle_source.sigrefs",
            "Radicle signed-reference entry bound exceeds this platform",
        )
    })?;
    let mut refs = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if refs.len() >= maximum_entries {
            return Err(domain(
                "radicle_source_sigrefs_bound",
                "radicle_source.sigrefs",
                "Radicle signed-reference entry count exceeds the request bound",
            ));
        }
        let (oid, name) = line.split_once(' ').ok_or_else(|| {
            domain(
                "radicle_source_sigrefs_format",
                "radicle_source.sigrefs",
                "Radicle signed-reference line has an invalid shape",
            )
        })?;
        let _validated = RefString::try_from(name.to_string()).map_err(|_| {
            domain(
                "radicle_source_sigrefs_name",
                "radicle_source.sigrefs",
                "Radicle signed-reference name is invalid",
            )
        })?;
        let oid = parse_oid(oid, "radicle_source.sigrefs")?;
        if oid.is_zero() || refs.insert(name.to_string(), oid).is_some() {
            return Err(domain(
                "radicle_source_sigrefs_entry",
                "radicle_source.sigrefs",
                "Radicle signed-reference entry is zero or duplicated",
            ));
        }
    }
    if refs.is_empty() {
        return Err(domain(
            "radicle_source_sigrefs_empty",
            "radicle_source.sigrefs",
            "Radicle signed-reference map is empty",
        ));
    }
    let mut canonical = Vec::new();
    for (name, oid) in &refs {
        canonical.extend_from_slice(oid.to_string().as_bytes());
        canonical.push(b' ');
        canonical.extend_from_slice(name.as_bytes());
        canonical.push(b'\n');
    }
    Ok((refs, canonical))
}

// r[impl kiln.radicle.shell_source]
pub(super) fn verify_signed_refs(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    repository_id: RepoId,
    signer: &PublicKey,
    expected_sigrefs: Oid,
    expected_revision: Oid,
) -> PortResult<VerifiedSignedRefs> {
    let namespaced_reference = sigrefs_reference(profile);
    let sigrefs_commit = reference_target(repository, &namespaced_reference)?;
    if sigrefs_commit != expected_sigrefs {
        return Err(domain(
            "radicle_source_sigrefs_substitution",
            "radicle_source.sigrefs",
            "Radicle signed-reference commit differs from the exact request",
        ));
    }
    let commit = repository
        .find_commit(sigrefs_commit)
        .map_err(git_failure)?;
    let parent = match commit.parent_count() {
        0 => None,
        1 => Some(commit.parent_id(0).map_err(git_failure)?),
        _ => {
            return Err(domain(
                "radicle_source_sigrefs_history",
                "radicle_source.sigrefs",
                "Radicle signed-reference history is not linear",
            ));
        }
    };
    let refs_bytes = read_blob(repository, &commit, REFS_BLOB_PATH)?;
    let signature_bytes = read_blob(repository, &commit, SIGNATURE_BLOB_PATH)?;
    let (refs, canonical) = parse_refs(profile, &refs_bytes)?;
    let signature = RadicleSignature::try_from(signature_bytes.as_slice()).map_err(|_| {
        domain(
            "radicle_source_signature_format",
            "radicle_source.sigrefs",
            "Radicle signed-reference signature has an invalid shape",
        )
    })?;
    let verifier = VerifyingKey::try_from(signer).map_err(|_| {
        domain(
            "radicle_source_signer_key",
            "radicle_source.signer",
            "Radicle signer identity cannot verify signatures",
        )
    })?;
    verifier.verify(&canonical, &signature).map_err(|_| {
        domain(
            "radicle_source_signature",
            "radicle_source.sigrefs",
            "Radicle signed-reference signature verification failed",
        )
    })?;

    let identity_root_commit = refs.get(IDENTITY_ROOT_REFERENCE).copied().ok_or_else(|| {
        domain(
            "radicle_source_identity_root",
            "radicle_source.sigrefs",
            "Radicle signed references do not contain an identity root",
        )
    })?;
    let identity_commit = repository
        .find_commit(identity_root_commit)
        .map_err(git_failure)?;
    let identity_tree = identity_commit.tree().map_err(git_failure)?;
    let identity_entry = identity_tree
        .get_path(Path::new(IDENTITY_DOCUMENT_PATH))
        .map_err(git_failure)?;
    if identity_entry.kind() != Some(ObjectType::Blob) {
        return Err(domain(
            "radicle_source_identity_document",
            "radicle_source.identity",
            "Radicle identity document is not a blob",
        ));
    }
    if RepoId::from(identity_entry.id()) != repository_id {
        return Err(domain(
            "radicle_source_identity_substitution",
            "radicle_source.identity",
            "Radicle identity root differs from the requested RID",
        ));
    }

    let declared_parent = refs.get(SIGREFS_PARENT_REFERENCE).copied();
    match (parent, declared_parent) {
        (None, None) => {}
        (Some(actual), Some(declared)) if actual == declared => {}
        _ => {
            return Err(domain(
                "radicle_source_sigrefs_parent",
                "radicle_source.sigrefs",
                "Radicle signed-reference parent does not match its commit history",
            ));
        }
    }

    let signed_revision = refs
        .get(&profile.request.signed_reference)
        .copied()
        .ok_or_else(|| {
            domain(
                "radicle_source_signed_branch",
                "radicle_source.reference",
                "Requested branch is absent from the signed-reference map",
            )
        })?;
    if signed_revision != expected_revision {
        return Err(domain(
            "radicle_source_revision_substitution",
            "radicle_source.revision",
            "Signed branch revision differs from the exact request",
        ));
    }
    let namespaced_revision = reference_target(repository, &namespaced_source_reference(profile))?;
    if namespaced_revision != signed_revision {
        return Err(domain(
            "radicle_source_namespace_substitution",
            "radicle_source.reference",
            "Namespaced branch target differs from its signed revision",
        ));
    }
    repository
        .find_commit(signed_revision)
        .map_err(git_failure)?;
    Ok(VerifiedSignedRefs {
        sigrefs_commit,
        sigrefs_parent: parent,
        identity_root_commit,
        signed_revision,
        namespaced_reference,
    })
}

pub(super) fn parse_oid(value: &str, field: &str) -> PortResult<Oid> {
    Oid::from_str(value).map_err(|_| {
        domain(
            "radicle_source_oid",
            field,
            "Radicle source identity is not a valid Git object identity",
        )
    })
}
