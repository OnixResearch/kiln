//! Read-only Radicle patch reducer helper, isolated from Kiln's rusqlite link.

use std::path::PathBuf;
use std::str::FromStr;

use radicle::cob::patch::Patches;
use radicle::cob::store::access::ReadOnly;
use radicle::identity::RepoId;
use radicle::storage::git::Repository;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(storage_root), Some(repository_id), Some(patch_id), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        eprintln!("usage: kiln-radicle-patch STORAGE_ROOT REPOSITORY_ID PATCH_ID");
        std::process::exit(2);
    };
    match read_patch(PathBuf::from(storage_root), &repository_id, &patch_id) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
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
