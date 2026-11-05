//! Read-only Radicle patch reducer helper, isolated from Kiln's rusqlite link.

use std::path::PathBuf;
use std::str::FromStr;

use radicle::cob::cache::Update;
use radicle::cob::patch::{Patch, Patches, Verdict};
use radicle::cob::store::access::{ReadOnly, WriteAs};
use radicle::crypto::{Seed, SigningKey};
use radicle::identity::RepoId;
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
        Some("review") => {
            let (
                Some(storage_root),
                Some(repository_id),
                Some(patch_id),
                Some(revision_id),
                Some(verdict),
                Some(summary),
                None,
            ) = (
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
        "usage: kiln-radicle-patch read STORAGE_ROOT REPOSITORY_ID PATCH_ID\n       kiln-radicle-patch review STORAGE_ROOT REPOSITORY_ID PATCH_ID REVISION_ID accept|reject SUMMARY"
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
    let path = storage_root.join(rid.to_string());
    let repository = Repository::open(&path, rid).map_err(|_| "Radicle repository unavailable")?;
    let patches =
        Patches::open(&repository, ReadOnly).map_err(|_| "Radicle patch store unavailable")?;
    let id = radicle::cob::ObjectId::from_str(patch_id).map_err(|_| "invalid patch id")?;
    let patch = patches
        .get(&id)
        .map_err(|_| "Radicle patch reduction failed")?
        .ok_or("Radicle patch not found")?;
    Ok(serde_json::json!({
        "patch_id": patch_id,
        "state": format!("{:?}", patch.state()),
        "author": patch.author().id().to_string(),
        "revisions": patch.revisions().count(),
    }))
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
) -> Result<serde_json::Value, String> {
    let seed =
        std::env::var("RADICLE_SIGNER_SEED").map_err(|_| "RADICLE_SIGNER_SEED is required")?;
    let seed = hex::decode(seed.trim()).map_err(|_| "invalid signer seed")?;
    let seed: [u8; Seed::BYTES] = seed
        .try_into()
        .map_err(|_| "signer seed must be 32 bytes")?;
    let signer = SigningKey::from_seed(Seed::new(seed));
    let rid = RepoId::from_str(repository_id).map_err(|_| "invalid Radicle repository id")?;
    let path = storage_root.join(rid.to_string());
    let repository = Repository::open(&path, rid).map_err(|_| "Radicle repository unavailable")?;
    let mut patches = Patches::open(&repository, WriteAs::new(&signer))
        .map_err(|_| "Radicle patch store unavailable")?;
    let id = radicle::cob::ObjectId::from_str(patch_id).map_err(|_| "invalid patch id")?;
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
    let review = patch
        .review(
            revision.into(),
            verdict,
            Some(summary.to_owned()),
            Vec::new(),
        )
        .map_err(|_| "Radicle review append failed")?;
    Ok(serde_json::json!({"review_id": review.to_string()}))
}
