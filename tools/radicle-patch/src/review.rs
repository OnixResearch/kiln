//! Native reduction in an isolated ref snapshot; real publication uses Git ref
//! locks and expected-old CAS, including the signer's complete namespace.
use radicle::cob::ObjectId;
use radicle::crypto::PublicKey;
use radicle::storage::git::Repository;
use std::collections::BTreeMap;

#[derive(Clone, Eq, PartialEq)]
enum Target {
    Direct(radicle::git::raw::Oid),
    Symbolic(String),
}
type Refs = BTreeMap<String, Target>;
const MAX_SNAPSHOT_REFS: usize = 65_536;
const MAX_SNAPSHOT_REF_BYTES: usize = 16_777_216;

fn refs(repo: &Repository) -> Result<Refs, String> {
    let mut result = BTreeMap::new();
    let mut bytes = 0usize;
    for reference in repo.backend.references().map_err(|e| e.to_string())? {
        let reference = reference.map_err(|e| e.to_string())?;
        let name = reference.name().map_err(|_| "non-UTF8 native ref denied")?;
        bytes = bytes.checked_add(name.len()).ok_or("ref byte overflow")?;
        if result.len() >= MAX_SNAPSHOT_REFS || bytes > MAX_SNAPSHOT_REF_BYTES {
            return Err("native ref snapshot bound exceeded".into());
        }
        let target = match reference.target() {
            Some(oid) => Target::Direct(oid),
            None => {
                let target = reference
                    .symbolic_target()
                    .map_err(|e| e.to_string())?
                    .ok_or("native ref target unavailable")?;
                bytes = bytes.checked_add(target.len()).ok_or("ref byte overflow")?;
                if bytes > MAX_SNAPSHOT_REF_BYTES {
                    return Err("native ref snapshot byte bound exceeded".into());
                }
                Target::Symbolic(target.to_owned())
            }
        };
        result.insert(name.to_owned(), target);
    }
    Ok(result)
}

pub struct ReviewSnapshot {
    pub repository: Repository,
    _directory: tempfile::TempDir,
    before: Refs,
}
impl ReviewSnapshot {
    pub fn capture(original: &Repository) -> Result<Self, String> {
        let before = refs(original)?;
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let backend = radicle::git::raw::Repository::init_bare(directory.path())
            .map_err(|e| e.to_string())?;
        // Share immutable Git objects, never the mutable namespace refs. Newly
        // signed unreachable objects are harmless if publication later loses CAS.
        std::fs::remove_dir_all(directory.path().join("objects")).map_err(|e| e.to_string())?;
        let objects = std::fs::canonicalize(original.backend.path().join("objects"))
            .map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(objects, directory.path().join("objects"))
            .map_err(|e| e.to_string())?;
        for (name, target) in &before {
            match target {
                Target::Direct(oid) => {
                    backend
                        .reference(name, *oid, false, "native review snapshot")
                        .map_err(|e| e.to_string())?;
                }
                Target::Symbolic(target) => {
                    backend
                        .reference_symbolic(name, target, false, "native review snapshot")
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        let mut config = backend.config().map_err(|e| e.to_string())?;
        config
            .set_str("user.name", "Kiln native review")
            .map_err(|e| e.to_string())?;
        config
            .set_str("user.email", "review@kiln.invalid")
            .map_err(|e| e.to_string())?;
        drop(config);
        drop(backend);
        let repository =
            Repository::open(directory.path(), original.id).map_err(|e| e.to_string())?;
        if refs(original)? != before {
            return Err("native refs moved during snapshot capture".into());
        }
        Ok(Self {
            repository,
            _directory: directory,
            before,
        })
    }
    pub fn publish(
        &self,
        original: &Repository,
        patch: &ObjectId,
        signer: &PublicKey,
    ) -> Result<(), String> {
        let prefix = format!("refs/namespaces/{signer}/");
        let cob_ref = format!("{prefix}refs/cobs/xyz.radicle.patch/{patch}");
        let sigref = format!("{prefix}refs/rad/sigrefs");
        let own_before: Refs = self
            .before
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();
        let mut locked: std::collections::BTreeSet<_> = own_before.keys().cloned().collect();
        locked.insert(cob_ref.clone());
        locked.insert(sigref.clone());
        let mut transaction = original.backend.transaction().map_err(|e| e.to_string())?;
        for name in &locked {
            transaction
                .lock_ref(name)
                .map_err(|_| "own namespace ref publication is concurrently locked")?;
        }
        let own_current: Refs = refs(original)?
            .into_iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .collect();
        if own_current != own_before {
            return Err("own namespace moved before review publication CAS".into());
        }
        let signature =
            radicle::git::raw::Signature::now("Kiln native review", "review@kiln.invalid")
                .map_err(|e| e.to_string())?;
        for name in [&cob_ref, &sigref] {
            let target = self
                .repository
                .backend
                .refname_to_id(name)
                .map_err(|e| e.to_string())?;
            transaction
                .set_target(
                    name,
                    target,
                    Some(&signature),
                    "publish native review with expected-old CAS",
                )
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }
}
