//! Bounded, authenticated native COB history admission.
use radicle::cob::change::Storage as _;
use radicle::cob::{Entry, ObjectId, TypeName};
use radicle::storage::git::Repository;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub const MAX_HISTORY_ENTRIES: usize = 65_536;
pub const MAX_HISTORY_BYTES: usize = 536_870_912;
pub const MAX_HEADS: usize = 256;
const MAX_ENTRY_MEMBERS: usize = 256;
const MAX_METADATA_BYTES: usize = 1_048_576;

fn preflight(
    repository: &Repository,
    oid: radicle::git::Oid,
    remaining: usize,
) -> Result<(), String> {
    let odb = repository.backend.odb().map_err(|e| e.to_string())?;
    if odb.read_header(oid.into()).map_err(|e| e.to_string())?.0 > MAX_METADATA_BYTES {
        return Err("COB commit metadata bound exceeded".into());
    }
    let commit = repository
        .backend
        .find_commit(oid.into())
        .map_err(|e| e.to_string())?;
    let tree = commit.tree().map_err(|e| e.to_string())?;
    if tree.len() > MAX_ENTRY_MEMBERS {
        return Err("COB entry member bound exceeded".into());
    }
    let mut bytes = 0usize;
    for member in &tree {
        if member.kind() == Some(radicle::git::raw::ObjectType::Blob) {
            bytes = bytes
                .checked_add(odb.read_header(member.id()).map_err(|e| e.to_string())?.0)
                .ok_or("COB byte overflow")?;
            if bytes > remaining {
                return Err("COB history byte bound exceeded".into());
            }
        }
    }
    if let Ok(document) = tree.get_path(std::path::Path::new("embeds/radicle.json")) {
        if odb.read_header(document.id()).map_err(|e| e.to_string())?.0 > MAX_METADATA_BYTES {
            return Err("identity metadata bound exceeded".into());
        }
    }
    Ok(())
}

fn heads(
    repository: &Repository,
    typename: &TypeName,
    id: &ObjectId,
) -> Result<BTreeMap<String, (radicle::git::Oid, radicle::crypto::PublicKey)>, String> {
    let pattern = format!("refs/namespaces/*/refs/cobs/{typename}/{id}");
    let qualified = format!("refs/cobs/{typename}/{id}");
    let mut heads = BTreeMap::new();
    let mut count = 0usize;
    for reference in repository
        .backend
        .references_glob(&pattern)
        .map_err(|e| e.to_string())?
    {
        count += 1;
        if count > MAX_HEADS {
            return Err("COB head bound exceeded".into());
        }
        let reference = reference.map_err(|e| e.to_string())?;
        let target: radicle::git::Oid = reference.target().ok_or("symbolic COB ref denied")?.into();
        preflight(repository, target, MAX_HISTORY_BYTES)?;
        let namespace = reference
            .name()
            .map_err(|_| "invalid namespace ref")?
            .strip_prefix("refs/namespaces/")
            .and_then(|s| s.split('/').next())
            .ok_or("invalid namespace")?;
        let actor = radicle::crypto::PublicKey::from_str(namespace)
            .map_err(|_| "invalid namespace owner")?;
        let sigref = repository
            .backend
            .refname_to_id(&format!("refs/namespaces/{actor}/refs/rad/sigrefs"))
            .map_err(|_| "COB namespace has no signed refs")?;
        preflight(repository, sigref.into(), MAX_METADATA_BYTES)?;
        let signed = radicle::storage::refs::SignedRefs::load(actor, repository)
            .map_err(|_| "COB namespace signature invalid")?
            .ok_or("COB signed refs absent")?;
        if !signed
            .iter()
            .any(|(name, oid)| name.as_str() == qualified && *oid == target)
        {
            return Err("COB ref differs from authenticated signed target".into());
        }
        heads.insert(
            reference
                .name()
                .map_err(|_| "invalid namespace ref")?
                .to_owned(),
            (target, actor),
        );
    }
    if heads.is_empty() {
        return Err("COB head set missing".into());
    }
    Ok(heads)
}

// r[impl kiln.radicle.multiwriter]
pub fn history(
    repository: &Repository,
    typename: &TypeName,
    id: &ObjectId,
) -> Result<(BTreeSet<String>, Vec<Entry>), String> {
    let max_entries = limit("KILN_HISTORY_ENTRIES", MAX_HISTORY_ENTRIES)?;
    let max_bytes = limit("KILN_HISTORY_BYTES", MAX_HISTORY_BYTES)?;
    let advertised = heads(repository, typename, id)?;
    let mut pending: Vec<_> = advertised.values().map(|(id, _)| *id).collect();
    let mut visited = BTreeSet::new();
    let mut entries = Vec::new();
    let mut children: BTreeMap<radicle::git::Oid, Vec<radicle::git::Oid>> = BTreeMap::new();
    let mut bytes = 0usize;
    while let Some(oid) = pending.pop() {
        if !visited.insert(oid) {
            continue;
        }
        if visited.len() > max_entries {
            return Err("COB history entry bound exceeded".into());
        }
        preflight(repository, oid, max_bytes.saturating_sub(bytes))?;
        let entry = repository.load(oid).map_err(|e| e.to_string())?;
        if !entry.valid_signatures() {
            return Err("COB change signature invalid".into());
        }
        if advertised
            .values()
            .any(|(tip, actor)| *tip == oid && actor != entry.author())
        {
            return Err("COB tip author differs from signed namespace owner".into());
        }
        if entry.manifest.type_name != *typename {
            return Err("COB history type mismatch".into());
        }
        if let Some(resource) = entry.resource {
            preflight(repository, resource, MAX_METADATA_BYTES)?;
        }
        for content in entry.contents() {
            bytes = bytes
                .checked_add(content.len())
                .ok_or("COB history byte overflow")?;
            if bytes > max_bytes {
                return Err("COB history byte bound exceeded".into());
            }
            if typename == &*radicle::cob::patch::TYPENAME {
                serde_json::from_slice::<radicle::cob::patch::Action>(content)
                    .map_err(|_| "opaque patch action denied")?;
            } else if typename == &*radicle::cob::issue::TYPENAME {
                serde_json::from_slice::<radicle::cob::issue::Action>(content)
                    .map_err(|_| "opaque issue action denied")?;
            } else {
                return Err("unsupported native COB type".into());
            }
        }
        if entry.parents.len() > MAX_HEADS {
            return Err("COB parent bound exceeded".into());
        }
        for parent in &entry.parents {
            children.entry(*parent).or_default().push(oid);
        }
        pending.extend(entry.parents.iter().copied());
        entries.push(entry);
    }
    let mut connected = BTreeSet::new();
    let mut pending = vec![**id];
    while let Some(parent) = pending.pop() {
        if connected.insert(parent) {
            if let Some(next) = children.get(&parent) {
                pending.extend(next.iter().copied());
            }
        }
    }
    if connected != visited {
        return Err("COB contains history disconnected from declared root".into());
    }
    if heads(repository, typename, id)? != advertised {
        return Err("COB head set moved during authenticated history load".into());
    }
    Ok((
        advertised.values().map(|(id, _)| id.to_string()).collect(),
        entries,
    ))
}
pub fn require_heads(expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> Result<(), String> {
    if expected.is_empty() || expected != actual {
        return Err("COB head set changed since preview".into());
    }
    Ok(())
}
fn limit(name: &str, maximum: usize) -> Result<usize, String> {
    match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => Ok(maximum),
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0 && *n <= maximum)
            .ok_or_else(|| "invalid COB history bound".into()),
        Err(_) => Err("invalid COB history bound".into()),
    }
}
