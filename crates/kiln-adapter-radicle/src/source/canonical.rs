//! Canonical Radicle source mode: verify every delegate namespace's signed
//! references and accept a revision only when heartwood's canonical computation
//! (vendored at the pinned revision) names it for the configured reference.
//!
//! The identity document is a reviewed, pinned input: this module parses the
//! exact document revision named by the request and requires every observing
//! delegate's signed references to embed identical bytes. It does not verify the
//! identity document's own revision history; that is recorded as a non-claim.

use std::collections::BTreeMap;
use std::path::Path;

use cob_git2 as git2;
use git2::{ObjectType, Oid, Repository};
use heartwood_canonical::git::canonical::Object;
use heartwood_canonical::git::canonical::rules::{RawPattern, RawRule, RawRules, Rules};
use heartwood_canonical::git::fmt::{Qualified, RefStr};
use heartwood_canonical::identity::Did;
use heartwood_canonical::identity::doc::{Delegates, Threshold};
use kiln_provider_api::PortResult;
use kiln_radicle_source_contract::{CanonicalDelegateObservation, CanonicalEvidence};
use radicle_core::RepoId;
use radicle_crypto::signature::Verifier as _;
use radicle_crypto::{PublicKey, Signature as RadicleSignature, VerifyingKey};

use super::error::{domain, git_failure};
use super::profile::RadicleSourceProfile;
use super::protocol::{parse_refs, read_blob};

const IDENTITY_DOCUMENT_PATH: &str = "embeds/radicle.json";
const IDENTITY_ROOT_REFERENCE: &str = "refs/rad/root";
const SIGREFS_REFERENCE: &str = "refs/rad/sigrefs";
const SIGREFS_PARENT_REFERENCE: &str = "refs/rad/sigrefs-parent";
const CREFS_PAYLOAD: &str = "xyz.radicle.crefs";

/// The pinned identity document, parsed from its canonical JSON.
#[derive(serde::Deserialize)]
struct IdentityDocument {
    delegates: Vec<String>,
    threshold: usize,
    #[serde(default)]
    payload: BTreeMap<String, serde_json::Value>,
}

struct DelegateVote {
    delegate: String,
    object: Oid,
}

fn delegate_namespace(delegate: &PublicKey) -> String {
    delegate.to_string()
}

fn verify_delegate(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    repository_id: RepoId,
    delegate: &PublicKey,
    reference: &str,
    document: &[u8],
) -> PortResult<Option<DelegateVote>> {
    let key = delegate_namespace(delegate);
    let namespaced_sigrefs = format!("refs/namespaces/{key}/{SIGREFS_REFERENCE}");
    let Some(sigrefs_commit) = repository
        .find_reference(&namespaced_sigrefs)
        .ok()
        .and_then(|found| found.target())
    else {
        // A delegate that published nothing has no vote.
        return Ok(None);
    };
    let commit = repository
        .find_commit(sigrefs_commit)
        .map_err(git_failure)?;
    if commit.parent_count() > 1 {
        return Err(domain(
            "radicle_source_canonical_history",
            "radicle_source.sigrefs",
            "Radicle signed-reference history is not linear",
        ));
    }
    let parent = match commit.parent_count() {
        0 => None,
        1 => Some(commit.parent_id(0).map_err(git_failure)?),
        _ => unreachable!("parent count checked above"),
    };
    let refs_bytes = read_blob(repository, &commit, "refs")?;
    let signature_bytes = read_blob(repository, &commit, "signature")?;
    let (refs, canonical) = parse_refs(profile, &refs_bytes)?;
    let signature = RadicleSignature::try_from(signature_bytes.as_slice()).map_err(|_| {
        domain(
            "radicle_source_canonical_signature",
            "radicle_source.sigrefs",
            "Radicle delegate signed-reference signature has an invalid shape",
        )
    })?;
    let verifier = VerifyingKey::try_from(delegate).map_err(|_| {
        domain(
            "radicle_source_canonical_signer",
            "radicle_source.signer",
            "Radicle delegate identity cannot verify signatures",
        )
    })?;
    verifier.verify(&canonical, &signature).map_err(|_| {
        domain(
            "radicle_source_canonical_signature",
            "radicle_source.sigrefs",
            "Radicle delegate signed-reference signature verification failed",
        )
    })?;
    let root = refs.get(IDENTITY_ROOT_REFERENCE).copied().ok_or_else(|| {
        domain(
            "radicle_source_canonical_identity",
            "radicle_source.sigrefs",
            "Radicle delegate signed references omit the identity root",
        )
    })?;
    let root_commit = repository.find_commit(root).map_err(git_failure)?;
    let root_tree = root_commit.tree().map_err(git_failure)?;
    let root_entry = root_tree
        .get_path(Path::new(IDENTITY_DOCUMENT_PATH))
        .map_err(git_failure)?;
    if root_entry.kind() != Some(ObjectType::Blob) || RepoId::from(root_entry.id()) != repository_id
    {
        return Err(domain(
            "radicle_source_canonical_identity",
            "radicle_source.identity",
            "Radicle delegate identity root differs from the requested repository",
        ));
    }
    let embedded = read_blob(repository, &commit, IDENTITY_DOCUMENT_PATH)?;
    if embedded != document {
        return Err(domain(
            "radicle_source_canonical_identity_revision",
            "radicle_source.identity",
            "Radicle delegate signed a different identity document revision",
        ));
    }
    let declared_parent = refs.get(SIGREFS_PARENT_REFERENCE).copied();
    match (parent, declared_parent) {
        (None, None) => {}
        (Some(actual), Some(declared)) if actual == declared => {}
        _ => {
            return Err(domain(
                "radicle_source_canonical_history",
                "radicle_source.sigrefs",
                "Radicle delegate signed-reference parent does not match its commit history",
            ));
        }
    }
    let Some(signed_object) = refs.get(reference).copied() else {
        // A delegate that has not signed this reference has no vote.
        return Ok(None);
    };
    let namespaced_reference = format!("refs/namespaces/{key}/{reference}");
    let observed = repository
        .find_reference(&namespaced_reference)
        .ok()
        .and_then(|found| found.target())
        .ok_or_else(|| {
            domain(
                "radicle_source_canonical_namespace",
                "radicle_source.reference",
                "Radicle delegate namespace reference is missing",
            )
        })?;
    if observed != signed_object {
        return Err(domain(
            "radicle_source_canonical_namespace",
            "radicle_source.reference",
            "Radicle delegate namespace target differs from its signed reference",
        ));
    }
    Ok(Some(DelegateVote {
        delegate: Did::from(delegate).encode(),
        object: signed_object,
    }))
}

fn qualified(reference: &str) -> PortResult<Qualified<'static>> {
    let refstr = RefStr::try_from_str(reference).map_err(|_| {
        domain(
            "radicle_source_canonical_reference",
            "radicle_source.reference",
            "Canonical reference is not a valid Git reference",
        )
    })?;
    Qualified::from_refstr(refstr.to_owned()).ok_or_else(|| {
        domain(
            "radicle_source_canonical_reference",
            "radicle_source.reference",
            "Canonical reference is not fully qualified",
        )
    })
}

// r[impl kiln.radicle.canonical_source]
// r[impl kiln.radicle.canonical_source.identity]
pub(super) fn verify_canonical(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    repository_id: RepoId,
) -> PortResult<CanonicalEvidence> {
    let request = profile.request.canonical.as_ref().ok_or_else(|| {
        domain(
            "radicle_source_canonical_request",
            "radicle_source.request.canonical",
            "Canonical source verification requires canonical request inputs",
        )
    })?;
    let identity_commit = super::protocol::parse_oid(
        &request.identity_commit,
        "radicle_source.request.canonical.identity_commit",
    )?;
    let commit = repository
        .find_commit(identity_commit)
        .map_err(git_failure)?;
    let document = read_blob(repository, &commit, IDENTITY_DOCUMENT_PATH)?;
    let parsed: IdentityDocument = serde_json::from_slice(&document).map_err(|_| {
        domain(
            "radicle_source_canonical_identity",
            "radicle_source.identity",
            "Radicle identity document is not valid canonical JSON",
        )
    })?;
    let mut dids = Vec::with_capacity(parsed.delegates.len());
    let mut keys = Vec::with_capacity(parsed.delegates.len());
    for delegate in &parsed.delegates {
        let did = Did::decode(delegate).map_err(|_| {
            domain(
                "radicle_source_canonical_delegate",
                "radicle_source.identity.delegates",
                "Radicle identity delegate is not a valid did:key",
            )
        })?;
        dids.push(did);
        keys.push(PublicKey::from(did));
    }
    let delegates = Delegates::new(dids).map_err(|_| {
        domain(
            "radicle_source_canonical_delegate",
            "radicle_source.identity.delegates",
            "Radicle identity delegates are empty, duplicated or exceed the bound",
        )
    })?;
    let threshold = Threshold::new(parsed.threshold, &delegates).map_err(|_| {
        domain(
            "radicle_source_canonical_threshold",
            "radicle_source.identity.threshold",
            "Radicle identity threshold is invalid for its delegates",
        )
    })?;
    if !keys
        .iter()
        .any(|key| key.to_string() == profile.request.signer_node_id)
    {
        return Err(domain(
            "radicle_source_canonical_primary",
            "radicle_source.request.signer_node_id",
            "The requested signer is not a delegate of the pinned identity document",
        ));
    }

    let mut root = None;
    let mut votes = Vec::new();
    for key in &keys {
        let Some(vote) = verify_delegate(
            profile,
            repository,
            repository_id,
            key,
            &request.reference,
            &document,
        )?
        else {
            continue;
        };
        if let Some(existing) = root {
            if existing != vote_root(repository, key, profile)? {
                return Err(domain(
                    "radicle_source_canonical_root",
                    "radicle_source.identity",
                    "Radicle delegates disagree on the identity root",
                ));
            }
        } else {
            root = Some(vote_root(repository, key, profile)?);
        }
        votes.push(vote);
    }
    let root = root.ok_or_else(|| {
        domain(
            "radicle_source_canonical_votes",
            "radicle_source.reference",
            "No delegate signed the canonical reference",
        )
    })?;

    let reference = qualified(&request.reference)?;
    let mut raw = match parsed.payload.get(CREFS_PAYLOAD) {
        Some(value) => serde_json::from_value::<RawRules>(value.clone()).map_err(|_| {
            domain(
                "radicle_source_canonical_rules",
                "radicle_source.identity.crefs",
                "Radicle canonical reference rules are invalid",
            )
        })?,
        None => RawRules::default(),
    };
    if raw.matches(&reference).next().is_none() {
        let pattern = serde_json::from_value::<RawPattern>(serde_json::Value::String(
            request.reference.clone(),
        ))
        .map_err(|_| {
            domain(
                "radicle_source_canonical_rules",
                "radicle_source.identity.crefs",
                "Canonical reference cannot be represented as a rule pattern",
            )
        })?;
        let rule = serde_json::from_value::<RawRule>(serde_json::json!({
            "allow": "delegates",
            "threshold": parsed.threshold,
        }))
        .map_err(|_| {
            domain(
                "radicle_source_canonical_rules",
                "radicle_source.identity.crefs",
                "Default canonical rule cannot be constructed",
            )
        })?;
        raw.insert(pattern, rule);
    }
    let mut resolve = || delegates.clone();
    let rules = Rules::from_raw(raw, &mut resolve).map_err(|_| {
        domain(
            "radicle_source_canonical_rules",
            "radicle_source.identity.crefs",
            "Radicle canonical reference rules failed validation",
        )
    })?;
    let prepared = rules.canonical(reference, repository).ok_or_else(|| {
        domain(
            "radicle_source_canonical_rules",
            "radicle_source.identity.crefs",
            "No reviewed canonical rule matches the configured reference",
        )
    })?;
    let prepared = prepared.find_objects().map_err(|_| {
        domain(
            "radicle_source_canonical_objects",
            "radicle_source.reference",
            "Canonical computation could not find delegate reference objects",
        )
    })?;
    let quorum = prepared.quorum().map_err(|_| {
        domain(
            "radicle_source_canonical_quorum",
            "radicle_source.reference",
            "Delegates have no canonical agreement on the configured reference",
        )
    })?;
    let (object, object_kind) = match quorum.object {
        Object::Commit { id } => (id, "commit"),
        Object::Tag { id } => (id, "tag"),
    };
    let object = Oid::from_str(&object.to_string()).map_err(|_| {
        domain(
            "radicle_source_canonical_object",
            "radicle_source.reference",
            "Canonical object identity is not a valid Git object identity",
        )
    })?;
    let revision = repository
        .find_object(object, None)
        .map_err(git_failure)?
        .peel_to_commit()
        .map_err(git_failure)?
        .id();
    if revision.to_string() != profile.request.expected_revision {
        return Err(domain(
            "radicle_source_canonical_revision",
            "radicle_source.revision",
            "Requested revision is not the delegate-canonical reference target",
        ));
    }
    votes.sort_by(|left, right| left.delegate.cmp(&right.delegate));
    Ok(CanonicalEvidence {
        identity_commit: request.identity_commit.clone(),
        identity_root: root.to_string(),
        threshold: u64::try_from(usize::from(threshold)).map_err(|_| {
            domain(
                "radicle_source_canonical_threshold",
                "radicle_source.identity.threshold",
                "Canonical threshold exceeds the supported range",
            )
        })?,
        delegates: parsed.delegates,
        reference: request.reference.clone(),
        object: object.to_string(),
        object_kind: object_kind.to_owned(),
        revision: revision.to_string(),
        observations: votes
            .into_iter()
            .map(|vote| CanonicalDelegateObservation {
                delegate: vote.delegate,
                object: vote.object.to_string(),
            })
            .collect(),
    })
}

fn vote_root(
    repository: &Repository,
    key: &PublicKey,
    profile: &RadicleSourceProfile,
) -> PortResult<Oid> {
    let namespaced_sigrefs = format!("refs/namespaces/{key}/{SIGREFS_REFERENCE}");
    let sigrefs = repository
        .find_reference(&namespaced_sigrefs)
        .and_then(|found| {
            found
                .target()
                .ok_or_else(|| git2::Error::from_str("symbolic signed refs"))
        })
        .map_err(git_failure)?;
    let commit = repository.find_commit(sigrefs).map_err(git_failure)?;
    let (refs, _) = parse_refs(profile, &read_blob(repository, &commit, "refs")?)?;
    refs.get(IDENTITY_ROOT_REFERENCE).copied().ok_or_else(|| {
        domain(
            "radicle_source_canonical_identity",
            "radicle_source.sigrefs",
            "Radicle delegate signed references omit the identity root",
        )
    })
}
