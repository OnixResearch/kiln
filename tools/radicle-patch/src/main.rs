//! Read-only Radicle patch reducer helper, isolated from Kiln's rusqlite link.

mod history;
mod review;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;

use radicle::cob::cache::Update;
use radicle::cob::patch::{Patch, Patches, Verdict};
use radicle::cob::store::access::{ReadOnly, WriteAs};
use radicle::crypto::{Seed, SigningKey};
use radicle::identity::RepoId;
use radicle::storage::ReadRepository as _;
use radicle::storage::git::Repository;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("read") => {
            let (Some(storage_root), Some(repository_id), Some(patch_id), None) =
                (args.next(), args.next(), args.next(), args.next())
            else {
                usage();
            };
            match read_patch(PathBuf::from(storage_root), &repository_id, &patch_id) {
                Ok(value) => println!("{value}"),
                Err(error) => fail(&error),
            }
        }
        Some("read-issue") => {
            let (Some(root), Some(rid), Some(id), None) =
                (args.next(), args.next(), args.next(), args.next())
            else {
                usage()
            };
            match read_issue(PathBuf::from(root), &rid, &id) {
                Ok(value) => println!("{value}"),
                Err(error) => fail(&error),
            }
        }
        Some("review") => {
            let (
                Some(storage_root),
                Some(repository_id),
                Some(patch_id),
                Some(revision_id),
                Some(verdict),
                Some(summary),
                Some(expected_heads),
                None,
            ) = (
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
            )
            else {
                usage();
            };
            match review_patch(
                PathBuf::from(storage_root),
                &repository_id,
                &patch_id,
                &revision_id,
                &verdict,
                &summary,
                &serde_json::from_str::<BTreeSet<String>>(&expected_heads)
                    .unwrap_or_else(|_| fail("invalid expected head set")),
            ) {
                Ok(value) => println!("{value}"),
                Err(error) => fail(&error),
            }
        }
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: kiln-radicle-patch read|read-issue STORAGE_ROOT REPOSITORY_ID OBJECT_ID\n       kiln-radicle-patch review STORAGE_ROOT REPOSITORY_ID PATCH_ID REVISION_ID accept|reject SUMMARY EXPECTED_HEADS_JSON"
    );
    std::process::exit(2);
}

fn fail(error: &str) -> ! {
    eprintln!("{error}");
    std::process::exit(1);
}

fn read_patch(
    storage_root: PathBuf,
    repository_id: &str,
    patch_id: &str,
) -> Result<serde_json::Value, String> {
    let rid = RepoId::from_str(repository_id).map_err(|_| "invalid Radicle repository id")?;
    let path = storage_root.join(rid.canonical());
    let repository = Repository::open(&path, rid).map_err(|_| "Radicle repository unavailable")?;
    let patches =
        Patches::open(&repository, ReadOnly).map_err(|_| "Radicle patch store unavailable")?;
    let id = radicle::cob::ObjectId::from_str(patch_id).map_err(|_| "invalid patch id")?;
    let (heads, _) = history::history(&repository, &radicle::cob::patch::TYPENAME, &id)?;
    let patch = patches
        .get(&id)
        .map_err(|_| "Radicle patch reduction failed")?
        .ok_or("Radicle patch not found")?;
    let (after, _) = history::history(&repository, &radicle::cob::patch::TYPENAME, &id)?;
    history::require_heads(&heads, &after)?;
    let revisions = patch
        .revisions()
        .map(|(id, revision)| {
            serde_json::json!({
                "id": id.to_string(),
                "head": revision.head().to_string(),
                "base": revision.base().to_string(),
                "description": revision.description(),
                "author": revision.author().public_key().to_string(),
                "signed_source": signed_source(&repository, revision),
                "reviews": patch.reviews_of(id).map(|(id, review)| serde_json::json!({
                    "id": id.to_string(), "review": review,
                    "author": review.author().public_key().to_string(),
                    "verdict": review.verdict(), "summary": review.summary(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "patch_id": patch_id,
        "heads": heads,
        "native": patch,
        "delegates": repository.delegates().map_err(|e| e.to_string())?
            .iter().map(|did| did.as_key().to_string()).collect::<Vec<_>>(),
        "merges": patch.merges().map(|(actor, merge)| serde_json::json!({
            "actor": actor.to_string(), "merge": merge,
        })).collect::<Vec<_>>(),
        "state": format!("{:?}", patch.state()),
        "author": patch.author().id().to_string(),
        "revisions": revisions,
    }))
}

fn read_issue(
    root: PathBuf,
    repository_id: &str,
    object_id: &str,
) -> Result<serde_json::Value, String> {
    let rid = RepoId::from_str(repository_id).map_err(|_| "invalid repository id")?;
    let repository =
        Repository::open(root.join(rid.canonical()), rid).map_err(|e| e.to_string())?;
    let id = radicle::cob::ObjectId::from_str(object_id).map_err(|_| "invalid issue id")?;
    let (heads, _) = history::history(&repository, &radicle::cob::issue::TYPENAME, &id)?;
    let issues =
        radicle::cob::issue::Issues::open(&repository, ReadOnly).map_err(|e| e.to_string())?;
    let issue = issues
        .get(&id)
        .map_err(|e| e.to_string())?
        .ok_or("issue not found")?;
    let (after, _) = history::history(&repository, &radicle::cob::issue::TYPENAME, &id)?;
    history::require_heads(&heads, &after)?;
    Ok(serde_json::json!({"issue_id": object_id, "heads": heads, "native": issue}))
}

/// The signed patch COB itself has the base and oid as related Git parents.
/// Also accept signed branches; never trust an unsigned namespace ref.
fn signed_source(
    repository: &Repository,
    revision: &radicle::cob::patch::Revision,
) -> Option<serde_json::Value> {
    let author = revision.author().public_key();
    let signed = radicle::storage::refs::SignedRefs::load(*author, repository).ok()??;
    for (reference, oid) in signed.iter() {
        let name = reference.as_str();
        if !(name.starts_with("refs/heads/") || name.starts_with("refs/cobs/xyz.radicle.patch/")) {
            continue;
        }
        let actual = repository
            .backend
            .refname_to_id(&format!("refs/namespaces/{author}/{name}"))
            .ok();
        if actual != Some((*oid).into()) {
            continue;
        }
        let head = revision.head();
        if *oid == head
            || repository
                .backend
                .graph_descendant_of((*oid).into(), head.into())
                .unwrap_or(false)
        {
            return Some(serde_json::json!({"reference": name, "tip": oid.to_string()}));
        }
    }
    None
}

#[derive(Debug)]
struct NoCacheError;
impl std::fmt::Display for NoCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cache disabled")
    }
}
impl std::error::Error for NoCacheError {}

#[derive(Default)]
struct NoCache;
impl Update<Patch> for NoCache {
    type Out = ();
    type UpdateError = NoCacheError;
    fn update(
        &mut self,
        _rid: &RepoId,
        _id: &radicle::cob::ObjectId,
        _object: &Patch,
    ) -> Result<Self::Out, Self::UpdateError> {
        Ok(())
    }
}

fn review_patch(
    storage_root: PathBuf,
    repository_id: &str,
    patch_id: &str,
    revision_id: &str,
    verdict: &str,
    summary: &str,
    expected_heads: &BTreeSet<String>,
) -> Result<serde_json::Value, String> {
    let seed =
        std::env::var("RADICLE_SIGNER_SEED").map_err(|_| "RADICLE_SIGNER_SEED is required")?;
    let seed = hex::decode(seed.trim()).map_err(|_| "invalid signer seed")?;
    let seed: [u8; Seed::BYTES] = seed
        .try_into()
        .map_err(|_| "signer seed must be 32 bytes")?;
    let signer = SigningKey::from_seed(Seed::new(seed));
    let rid = RepoId::from_str(repository_id).map_err(|_| "invalid Radicle repository id")?;
    let path = storage_root.join(rid.canonical());
    let repository = Repository::open(&path, rid).map_err(|_| "Radicle repository unavailable")?;
    append_review(
        &repository,
        patch_id,
        revision_id,
        verdict,
        summary,
        expected_heads,
        &signer,
    )
}

/// Re-check the exact observed refs at the cryptographic signing boundary.
/// Native store signing then maintains sigrefs; that second signature follows
/// our own accepted write and must not be mistaken for a foreign head move.
struct HeadBoundSigner<'a> {
    repository: &'a Repository,
    patch: radicle::cob::ObjectId,
    expected: &'a BTreeSet<String>,
    signer: &'a SigningKey,
    signed_change: std::cell::Cell<bool>,
}
impl AsRef<radicle::crypto::PublicKey> for HeadBoundSigner<'_> {
    fn as_ref(&self) -> &radicle::crypto::PublicKey {
        self.signer.as_ref()
    }
}
impl radicle::crypto::signature::Keypair for HeadBoundSigner<'_> {
    type VerifyingKey = radicle::crypto::VerifyingKey;
    fn verifying_key(&self) -> Self::VerifyingKey {
        radicle::crypto::signature::Keypair::verifying_key(self.signer)
    }
}
impl radicle::crypto::signature::Signer<radicle::crypto::Signature> for HeadBoundSigner<'_> {
    fn try_sign(
        &self,
        message: &[u8],
    ) -> Result<radicle::crypto::Signature, radicle::crypto::signature::Error> {
        if !self.signed_change.get() {
            let (actual, _) =
                history::history(self.repository, &radicle::cob::patch::TYPENAME, &self.patch)
                    .map_err(|_| radicle::crypto::signature::Error::new())?;
            history::require_heads(self.expected, &actual)
                .map_err(|_| radicle::crypto::signature::Error::new())?;
            self.signed_change.set(true);
        }
        radicle::crypto::signature::Signer::try_sign(self.signer, message)
    }
}
fn append_review(
    repository: &Repository,
    patch_id: &str,
    revision_id: &str,
    verdict: &str,
    summary: &str,
    expected_heads: &BTreeSet<String>,
    signer: &SigningKey,
) -> Result<serde_json::Value, String> {
    let id = radicle::cob::ObjectId::from_str(patch_id).map_err(|_| "invalid patch id")?;
    use fs2::FileExt as _;
    use radicle::crypto::Signer as _;
    let review_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(
            repository
                .backend
                .path()
                .join(format!("kiln-review-{id}-{}", signer.public_key())),
        )
        .map_err(|_| "own review lock unavailable")?;
    review_lock
        .try_lock_exclusive()
        .map_err(|_| "own review already in progress; refresh preview")?;
    let bound = HeadBoundSigner {
        repository,
        patch: id,
        expected: expected_heads,
        signer,
        signed_change: std::cell::Cell::new(false),
    };
    let snapshot = review::ReviewSnapshot::capture(repository)?;
    let mut patches = Patches::open(&snapshot.repository, WriteAs::new(&bound))
        .map_err(|_| "Radicle patch store unavailable")?;
    let (heads, _) = history::history(&repository, &radicle::cob::patch::TYPENAME, &id)?;
    history::require_heads(expected_heads, &heads)?;
    let revision =
        radicle::cob::EntryId::from_str(revision_id).map_err(|_| "invalid revision id")?;
    let verdict = match verdict {
        "accept" => Some(Verdict::Accept),
        "reject" => Some(Verdict::Reject),
        _ => return Err("verdict must be accept or reject".into()),
    };
    let mut cache = NoCache;
    let mut patch = patches
        .get_mut(&id, &mut cache)
        .map_err(|_| "Radicle patch reduction failed")?;
    if patch.revision(&revision.into()).is_none() {
        return Err("revision not found".into());
    }
    let (heads, _) = history::history(&repository, &radicle::cob::patch::TYPENAME, &id)?;
    history::require_heads(expected_heads, &heads)?;
    let review = patch
        .review(
            revision.into(),
            verdict,
            Some(summary.to_owned()),
            Vec::new(),
        )
        .map_err(|_| "Radicle review append failed")?;
    drop(patch);
    snapshot.publish(repository, &id, signer.public_key())?;
    Ok(serde_json::json!({"review_id": review.to_string()}))
}

#[cfg(test)]
mod multiwriter_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use radicle::cob::patch::MergeTarget;
    use radicle::cob::store::access::ReadOnly;
    use radicle::crypto::Signer as _;
    use radicle::storage::ReadStorage;
    use radicle::test::fixtures;

    #[test]
    fn upstream_patch_reduces_and_accepts_a_review() {
        let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
        let signer = SigningKey::from_seed(Seed::new([9; Seed::BYTES]));
        let storage = radicle::storage::git::Storage::open(
            directory.path().join("storage"),
            radicle::git::UserInfo {
                alias: radicle::node::Alias::new("Radcliff"),
                key: *signer.public_key(),
            },
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let (rid, _, _, head) =
            fixtures::project(directory.path().join("project"), &storage, &signer)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
        let repository = storage
            .repository(rid)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut patches = Patches::open(&repository, WriteAs::new(&signer))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let title = radicle::cob::Title::from_str("Fixture patch")
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut cache = NoCache;
        let patch = patches
            .create(
                title,
                "fixture description",
                MergeTarget::default(),
                head,
                head,
                &[],
                &mut cache,
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let patch_id = *patch.id();
        drop(patch);

        let read =
            Patches::open(&repository, ReadOnly).unwrap_or_else(|error| unreachable!("{error:?}"));
        let reduced = read
            .get(&patch_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .unwrap_or_else(|| unreachable!("fixture patch missing"));
        let revision = reduced
            .revisions()
            .next()
            .map(|(id, _)| id)
            .unwrap_or_else(|| unreachable!("fixture revision"));
        drop(read);

        let mut patches = Patches::open(&repository, WriteAs::new(&signer))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut cache = NoCache;
        let mut patch = patches
            .get_mut(&patch_id, &mut cache)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let review = patch
            .review(
                revision,
                Some(Verdict::Accept),
                Some("ok".into()),
                Vec::new(),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(!review.to_string().is_empty());
        drop(patch);

        let read =
            Patches::open(&repository, ReadOnly).unwrap_or_else(|error| unreachable!("{error:?}"));
        let reduced = read
            .get(&patch_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .unwrap_or_else(|| unreachable!("fixture patch missing"));
        assert_eq!(reduced.revisions().count(), 1);
        assert_eq!(reduced.reviews_of(revision).count(), 1);
    }
}
