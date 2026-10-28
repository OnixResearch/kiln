//! Native Git framing, signature verification, durable replay and atomic writes.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::str::FromStr;

use cob_git2::{ObjectType, Repository, RepositoryOpenFlags};
use fs2::FileExt;
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::change::{Storage as _, Template};
use radicle_cob::object::Storage as _;
use radicle_cob::{ObjectId, TypeName};
use radicle_crypto::{PublicKey, Seed, SigningKey, signature::Keypair as _};
use radicle_oid::Oid;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::Profile;
use super::model::{self, Command, Kind, Operation, Record};

const TYPES: [&str; 9] = [
    "issue",
    "project",
    "milestone",
    "cycle",
    "initiative",
    "team",
    "request",
    "view",
    "personal",
];
const TYPE_PREFIX: &str = "xyz.onixresearch.work.";
const MAX_REFS: usize = 16_384;
const MAX_OBJECTS: usize = 1024;
const MAX_OBJECT_OPERATIONS: usize = 128;
const MAX_OPERATIONS: usize = 8192;
const MAX_HISTORY_BYTES: usize = 16 * 1024 * 1024;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_TREE_BYTES: usize = 4096;
const MAX_OPERATION_BYTES: usize = 262_144;
const MAX_RECORD_BYTES: usize = 262_144;
const LOCK_NAME: &str = "kiln-work.lock";

#[derive(Debug)]
pub(super) enum Failure {
    Missing,
    Invalid,
    Conflict,
    Replay,
    Unavailable,
}

type Result<T, E = Failure> = std::result::Result<T, E>;

fn unavailable<T>(_: T) -> Failure {
    Failure::Unavailable
}

pub(super) fn signer(profile: &Profile) -> std::result::Result<SigningKey, String> {
    let bytes = Zeroizing::new(
        crate::worker::files::regular(&profile.signer_seed_file, 32, true)
            .map_err(|_| "work signer seed unavailable or unsafe")?,
    );
    let seed = Zeroizing::new(
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| "work signer seed must be exactly 32 raw bytes")?,
    );
    Ok(SigningKey::from_seed(Seed::new(*seed)))
}

pub(super) fn is_oid(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    sequence: u64,
    previous_operation_head: Option<String>,
    operation: Operation,
}

struct Event {
    id: String,
    head: String,
    type_name: String,
    envelope: Envelope,
}

struct Snapshot {
    records: BTreeMap<String, Record>,
    events: Vec<Event>,
    bytes: usize,
}

pub(super) struct Response {
    pub records: BTreeMap<String, Record>,
    pub written: Option<(Record, bool)>,
}

/// The anonymous file supplies libgit2's config backend without writing either
/// repository or host config, changing HEAD, or changing process environment.
struct Opened {
    repository: Repository,
    _identity: File,
    _lock: File,
}

fn open(profile: &Profile) -> Result<Opened> {
    use nix::fcntl::{OFlag, openat};
    use nix::sys::stat::Mode;
    let directory = crate::worker::files::directory(&profile.git_directory).map_err(unavailable)?;
    let directory_metadata = nix::sys::stat::fstat(&directory).map_err(unavailable)?;
    if directory_metadata.st_uid != nix::unistd::geteuid().as_raw()
        || directory_metadata.st_mode & 0o022 != 0
    {
        return Err(Failure::Unavailable);
    }
    let lock = File::from(
        openat(
            &directory,
            LOCK_NAME,
            OFlag::O_RDWR
                | OFlag::O_CREAT
                | OFlag::O_NOFOLLOW
                | OFlag::O_NONBLOCK
                | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(unavailable)?,
    );
    let metadata = lock.metadata().map_err(unavailable)?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(Failure::Unavailable);
    }
    // Acquired before *any* ref read. Every service process uses this same lock;
    // native CAS additionally detects a non-cooperating change to our own ref.
    lock.try_lock_exclusive().map_err(unavailable)?;
    let repository = Repository::open_ext(
        &profile.git_directory,
        RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::NO_DOTGIT,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .map_err(unavailable)?;
    if repository.path() != profile.git_directory {
        return Err(Failure::Unavailable);
    }
    let mut identity = File::from(
        rustix::fs::memfd_create("kiln-work-identity", rustix::fs::MemfdFlags::CLOEXEC)
            .map_err(unavailable)?,
    );
    identity
        .write_all(b"[user]\n\tname = Kiln Work Service\n\temail = work@kiln.invalid\n")
        .map_err(unavailable)?;
    let config_path = std::path::PathBuf::from(format!("/proc/self/fd/{}", identity.as_raw_fd()));
    let config = cob_git2::Config::open(&config_path).map_err(unavailable)?;
    repository.set_config(&config).map_err(unavailable)?;
    Ok(Opened {
        repository,
        _identity: identity,
        _lock: lock,
    })
}

fn object_header(
    repository: &Repository,
    id: cob_git2::Oid,
    kind: ObjectType,
    maximum: usize,
) -> Result<usize> {
    let (bytes, observed) = repository
        .odb()
        .map_err(unavailable)?
        .read_header(id)
        .map_err(unavailable)?;
    if observed != kind || bytes > maximum {
        return Err(Failure::Unavailable);
    }
    Ok(bytes)
}

/// Bound all bytes before passing an entry to the native decoder. The service
/// writes exactly a manifest and one operation, with no resource or embeds.
fn frame(repository: &Repository, id: Oid, total: &mut usize) -> Result<Option<Oid>> {
    *total += object_header(repository, id.into(), ObjectType::Commit, MAX_COMMIT_BYTES)?;
    let commit = repository.find_commit(id.into()).map_err(unavailable)?;
    if commit.parent_count() > 1 {
        return Err(Failure::Conflict);
    }
    *total += object_header(
        repository,
        commit.tree_id(),
        ObjectType::Tree,
        MAX_TREE_BYTES,
    )?;
    let tree = commit.tree().map_err(unavailable)?;
    if tree.len() != 2 {
        return Err(Failure::Unavailable);
    }
    for (name, maximum) in [("manifest", 4096), ("0", MAX_OPERATION_BYTES)] {
        let entry = tree.get_name(name).ok_or(Failure::Unavailable)?;
        if entry.filemode() != 0o100644 {
            return Err(Failure::Unavailable);
        }
        *total += object_header(repository, entry.id(), ObjectType::Blob, maximum)?;
    }
    if *total > MAX_HISTORY_BYTES {
        return Err(Failure::Unavailable);
    }
    Ok(commit.parent_ids().next().map(Oid::from))
}

fn supported(name: &str) -> bool {
    name.strip_prefix(TYPE_PREFIX)
        .is_some_and(|kind| TYPES.contains(&kind))
}

fn native_type(record: &Record) -> Result<TypeName> {
    TypeName::from_str(match record.kind {
        Kind::Issue => "xyz.onixresearch.work.issue",
        Kind::Project => "xyz.onixresearch.work.project",
        Kind::Milestone => "xyz.onixresearch.work.milestone",
        Kind::Cycle => "xyz.onixresearch.work.cycle",
        Kind::Initiative => "xyz.onixresearch.work.initiative",
        Kind::Team => "xyz.onixresearch.work.team",
        Kind::Request => "xyz.onixresearch.work.request",
        Kind::View => "xyz.onixresearch.work.view",
        Kind::Personal => "xyz.onixresearch.work.personal",
    })
    .map_err(unavailable)
}

fn scan(repository: &Repository, namespace: &PublicKey) -> Result<Vec<(String, String, Oid)>> {
    let mut objects = BTreeMap::new();
    let expected_namespace = namespace.to_string();
    for (index, reference) in repository.references().map_err(unavailable)?.enumerate() {
        if index >= MAX_REFS {
            return Err(Failure::Unavailable);
        }
        let reference = reference.map_err(unavailable)?;
        let name = reference.name().map_err(unavailable)?;
        // Bare COB refs cannot be silently ignored if they claim a work type.
        if let Some(rest) = name.strip_prefix("refs/cobs/") {
            if rest
                .split_once('/')
                .is_some_and(|(kind, _)| supported(kind))
            {
                return Err(Failure::Conflict);
            }
        }
        let Some(rest) = name.strip_prefix("refs/namespaces/") else {
            continue;
        };
        let Some((actor, rest)) = rest.split_once("/refs/cobs/") else {
            continue;
        };
        let Some((kind, id)) = rest.split_once('/') else {
            continue;
        };
        if !supported(kind) {
            continue;
        }
        if !is_oid(id) {
            return Err(Failure::Unavailable);
        }
        if actor != expected_namespace {
            return Err(Failure::Conflict);
        }
        let head = reference.target().ok_or(Failure::Unavailable)?;
        if objects
            .insert(id.to_owned(), (kind.to_owned(), head.into()))
            .is_some()
        {
            return Err(Failure::Conflict);
        }
        if objects.len() > MAX_OBJECTS {
            return Err(Failure::Unavailable);
        }
    }
    Ok(objects
        .into_iter()
        .map(|(id, (kind, head))| (kind, id, head))
        .collect())
}

fn read(repository: &Repository, namespace: &PublicKey) -> Result<Snapshot> {
    let store = GitCobStore::new(repository);
    let mut bytes = 0;
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();
    for (type_name, id, tip) in scan(repository, namespace)? {
        let mut head = tip;
        let mut depth = 0;
        loop {
            depth += 1;
            if depth > MAX_OBJECT_OPERATIONS || events.len() >= MAX_OPERATIONS || !seen.insert(head)
            {
                return Err(Failure::Unavailable);
            }
            let parent = frame(repository, head, &mut bytes)?;
            let entry = store.load(head).map_err(unavailable)?;
            if !entry.valid_signatures()
                || entry.author() != namespace
                || entry.manifest.type_name.to_string() != type_name
                || usize::from(entry.manifest.version) != 1
                || entry.resource.is_some()
                || !entry.related.is_empty()
                || entry.parents != parent.into_iter().collect::<Vec<_>>()
                || !entry.contents.tail.is_empty()
            {
                return Err(Failure::Unavailable);
            }
            let envelope: Envelope =
                serde_json::from_slice(&entry.contents.head).map_err(unavailable)?;
            // The verified service signature attests historical admission. Current
            // write policy applies only to new commands, not retained history.
            if envelope.schema != "kiln.work-operation.v1"
                || envelope.sequence == 0
                || !super::valid_principal(&envelope.operation.actor)
            {
                return Err(Failure::Unavailable);
            }
            let command = &envelope.operation.command;
            match parent {
                Some(parent)
                    if command.object_id.as_deref() == Some(id.as_str())
                        && command.expected_head.as_deref()
                            == Some(parent.to_string().as_str()) => {}
                None if head.to_string() == id
                    && command.object_id.is_none()
                    && command.expected_head.is_none() => {}
                _ => return Err(Failure::Unavailable),
            }
            events.push(Event {
                id: id.clone(),
                head: head.to_string(),
                type_name: type_name.clone(),
                envelope,
            });
            match parent {
                Some(parent) => head = parent,
                None => break,
            }
        }
    }
    events.sort_by_key(|event| event.envelope.sequence);
    let mut records = BTreeMap::new();
    let mut requests = BTreeSet::new();
    for (index, event) in events.iter().enumerate() {
        if event.envelope.sequence != index as u64 + 1
            || event.envelope.previous_operation_head.as_deref()
                != index
                    .checked_sub(1)
                    .and_then(|previous| events.get(previous))
                    .map(|previous| previous.head.as_str())
            || !requests.insert(event.envelope.operation.command.request_id.as_str())
        {
            return Err(Failure::Unavailable);
        }
        reduce(&mut records, event)?;
    }
    Ok(Snapshot {
        records,
        events,
        bytes,
    })
}

fn reduce(records: &mut BTreeMap<String, Record>, event: &Event) -> Result<()> {
    let mut record = model::apply(records.get(&event.id), &event.envelope.operation, records)
        .map_err(unavailable)?;
    if native_type(&record)?.to_string() != event.type_name {
        return Err(Failure::Unavailable);
    }
    record.id.clone_from(&event.id);
    record.head.clone_from(&event.head);
    if serde_json::to_vec(&record).map_err(unavailable)?.len() > MAX_RECORD_BYTES {
        return Err(Failure::Unavailable);
    }
    records.insert(event.id.clone(), record);
    Ok(())
}

pub(super) fn execute(
    profile: &Profile,
    command: Option<Command>,
    principal: &str,
    now_ms: i64,
) -> Result<Response> {
    let signer = signer(profile).map_err(unavailable)?;
    let namespace = *signer.verifying_key().public_key();
    let opened = open(profile)?;
    let repository = &opened.repository;
    let mut snapshot = read(repository, &namespace)?;
    let Some(command) = command else {
        return Ok(Response {
            records: snapshot.records,
            written: None,
        });
    };
    // Missing and private objects deliberately have the same public outcome.
    let current = match command.object_id.as_deref() {
        Some(id) => Some(
            snapshot
                .records
                .get(id)
                .filter(|record| model::visible(record, principal))
                .ok_or(Failure::Missing)?,
        ),
        None => None,
    };
    if let Some(original) = snapshot
        .events
        .iter()
        .find(|event| event.envelope.operation.command.request_id == command.request_id)
    {
        if original.envelope.operation.actor != principal
            || original.envelope.operation.command != command
        {
            return Err(Failure::Replay);
        }
        let mut historical = BTreeMap::new();
        for event in &snapshot.events {
            reduce(&mut historical, event)?;
            if event.envelope.sequence == original.envelope.sequence {
                let record = historical.remove(&event.id).ok_or(Failure::Unavailable)?;
                if !model::visible(&record, principal) {
                    return Err(Failure::Missing);
                }
                return Ok(Response {
                    records: snapshot.records,
                    written: Some((record, true)),
                });
            }
        }
        return Err(Failure::Unavailable);
    }
    if let Some(current) = current {
        if command.expected_head.as_deref() != Some(current.head.as_str()) {
            return Err(Failure::Conflict);
        }
    }
    let operation = Operation {
        actor: principal.to_owned(),
        recorded_at_ms: now_ms,
        command,
    };
    let mut record =
        model::apply(current, &operation, &snapshot.records).map_err(|_| Failure::Invalid)?;
    let type_name = native_type(&record)?;
    // A newly created model record has empty IDs; reserve both native SHA-1s
    // before committing so an accepted write is always replayable under bounds.
    if serde_json::to_vec(&record).map_err(unavailable)?.len()
        + (if current.is_none() { 80 } else { 0 })
        > MAX_RECORD_BYTES
        || snapshot.events.len() >= MAX_OPERATIONS
        || (current.is_none() && snapshot.records.len() >= MAX_OBJECTS)
        || current.is_some_and(|record| {
            snapshot
                .events
                .iter()
                .filter(|event| event.id == record.id)
                .count()
                >= MAX_OBJECT_OPERATIONS
        })
    {
        return Err(Failure::Unavailable);
    }
    let envelope = Envelope {
        schema: "kiln.work-operation.v1".into(),
        sequence: snapshot.events.len() as u64 + 1,
        previous_operation_head: snapshot.events.last().map(|event| event.head.clone()),
        operation,
    };
    let payload = serde_json::to_vec(&envelope).map_err(unavailable)?;
    if payload.len() > MAX_OPERATION_BYTES
        || snapshot.bytes + payload.len() + MAX_COMMIT_BYTES + MAX_TREE_BYTES + 4096
            > MAX_HISTORY_BYTES
    {
        return Err(Failure::Unavailable);
    }
    let parent = current
        .map(|record| Oid::from_str(&record.head).map_err(unavailable))
        .transpose()?;
    let store = match parent {
        Some(parent) => GitCobStore::with_expected_ref(repository, parent),
        None => GitCobStore::new(repository),
    };
    let entry = store
        .store(
            None,
            Vec::new(),
            &signer,
            Template {
                type_name: type_name.clone(),
                tips: parent.into_iter().collect(),
                message: "Kiln authenticated application work operation".into(),
                embeds: Vec::new(),
                contents: NonEmpty::new(payload),
            },
        )
        .map_err(unavailable)?;
    let object_id = match current {
        Some(record) => ObjectId::from_str(&record.id).map_err(unavailable)?,
        None => ObjectId::from(entry.id),
    };
    store
        .update(&namespace, &type_name, &object_id, &entry.id)
        .map_err(unavailable)?;
    record.id = object_id.to_string();
    record.head = entry.id.to_string();
    snapshot.records.insert(record.id.clone(), record.clone());
    Ok(Response {
        records: snapshot.records,
        written: Some((record, false)),
    })
}
