//! Hosted operations: complete SQLite snapshots, scoped incident admission and owned content.
use crate::Actor;
use base64::Engine;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path, PathBuf},
    time::Instant,
};

const SCHEMA: &str = "kiln.hosted.v1";
const MAX_TEXT: usize = 4096;
const MAX_LIST: usize = 200;
const MAX_TABLES: usize = 256;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const BACKUP_STEP_PAGES: i32 = 128;
const MAX_DATABASE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_CONTENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REPLAY_STORES: usize = 128;
const MAX_SCHEMA_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationsProfile {
    pub service_id: String,
    pub backup_root: PathBuf,
    pub restore_root: PathBuf,
    pub content_root: PathBuf,
    #[serde(default)]
    pub replay_roots: Vec<PathBuf>,
    pub maximum_database_bytes: u64,
    pub maximum_content_bytes: u64,
    pub maximum_content_records: u64,
    pub maximum_backup_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub format: String,
    pub service_id: String,
    pub schema: String,
    pub backup_id: String,
    pub created_at_ms: u64,
    pub database_bytes: u64,
    pub database_blake3: String,
    pub schema_blake3: String,
    pub table_rows: BTreeMap<String, u64>,
    pub replay_stores: Vec<ReplaySnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplaySnapshot {
    pub original_path: PathBuf,
    pub snapshot_name: String,
    pub database_bytes: u64,
    pub database_blake3: String,
    pub schema_blake3: String,
    pub table_rows: BTreeMap<String, u64>,
}
impl BackupManifest {
    pub fn identity(&self) -> Result<String, String> {
        crate::identity(self)
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn integer(v: u64) -> Result<i64, String> {
    i64::try_from(v).map_err(err)
}
fn name(s: &str) -> Result<(), String> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid single-component identity".into());
    }
    Ok(())
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    let s = v
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {key}"))?;
    if s.is_empty() || s.len() > MAX_TEXT || s.contains('\0') {
        return Err(format!("invalid {key}"));
    }
    Ok(s)
}
fn number(v: &Value, key: &str) -> Result<u64, String> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing unsigned {key}"))
}
fn check_fields(v: &Value, fields: &[&str]) -> Result<(), String> {
    let obj = v.as_object().ok_or("expected object")?;
    if obj.keys().any(|k| !fields.contains(&k.as_str())) {
        return Err("unknown operation field".into());
    }
    Ok(())
}
fn root(path: &Path) -> Result<OwnedFd, String> {
    if !path.is_absolute() {
        return Err("configured root must be absolute".into());
    }
    let mut fd = fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(err)?;
    let mut components = 0;
    for c in path.components() {
        match c {
            Component::RootDir => {}
            Component::Normal(part) => {
                fd = fs::openat(
                    &fd,
                    part,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(err)?;
                components += 1;
            }
            _ => return Err("root traversal is forbidden".into()),
        }
    }
    if components == 0 {
        return Err("filesystem root is not an operations root".into());
    }
    let stat = fs::fstat(&fd).map_err(err)?;
    if stat.st_mode & 0o777 != 0o700 {
        return Err("operations roots must have mode 0700".into());
    }
    Ok(fd)
}
fn fd_path(fd: &OwnedFd, leaf: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{leaf}", fd.as_raw_fd()))
}
fn child(dir: &OwnedFd, id: &str, create: bool) -> Result<OwnedFd, String> {
    name(id)?;
    if create {
        fs::mkdirat(dir, id, Mode::from_raw_mode(0o700)).map_err(err)?;
    }
    let fd = fs::openat(
        dir,
        id,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(err)?;
    if fs::fstat(&fd).map_err(err)?.st_mode & 0o777 != 0o700 {
        return Err("snapshot directory must be private".into());
    }
    Ok(fd)
}
fn open_read(dir: &OwnedFd, leaf: &str) -> Result<File, String> {
    let fd = fs::openat(
        dir,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(err)?;
    let file = File::from(fd);
    let m = file.metadata().map_err(err)?;
    if !m.is_file() || m.nlink() != 1 {
        return Err("expected singly-linked regular file".into());
    }
    Ok(file)
}
fn create_file(dir: &OwnedFd, leaf: &str) -> Result<File, String> {
    fs::openat(
        dir,
        leaf,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(err)
}
fn validate_profile(p: &OperationsProfile) -> Result<(), String> {
    if p.service_id.is_empty()
        || p.service_id.len() > 128
        || p.maximum_database_bytes == 0
        || p.maximum_database_bytes > MAX_DATABASE_BYTES
        || p.maximum_content_bytes == 0
        || p.maximum_content_bytes > MAX_CONTENT_BYTES
        || p.maximum_content_records == 0
        || p.maximum_content_records > 1_000_000
        || p.maximum_backup_ms == 0
        || p.maximum_backup_ms > 300_000
    {
        return Err("invalid bounded operations profile".into());
    }
    if p.replay_roots.len() > MAX_REPLAY_STORES {
        return Err("replay root bound exceeded".into());
    }
    let mut roots = vec![
        root(&p.backup_root)?,
        root(&p.restore_root)?,
        root(&p.content_root)?,
    ];
    for path in &p.replay_roots {
        roots.push(root(path)?);
    }
    for (i, entry) in roots.iter().enumerate() {
        for other in roots.iter().take(i) {
            let a = fs::fstat(entry).map_err(err)?;
            let b = fs::fstat(other).map_err(err)?;
            if a.st_dev == b.st_dev && a.st_ino == b.st_ino {
                return Err("operations roots must be distinct".into());
            }
        }
    }
    Ok(())
}
fn hash_file(file: &mut File, max: u64) -> Result<(u64, String), String> {
    let mut h = blake3::Hasher::new();
    let mut count = 0u64;
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(err)?;
        if n == 0 {
            break;
        }
        count = count.checked_add(n as u64).ok_or("file size overflow")?;
        if count > max {
            return Err("file exceeds profile bound".into());
        }
        h.update(buf.get(..n).ok_or("read exceeded buffer length")?);
    }
    Ok((count, h.finalize().to_hex().to_string()))
}
fn inspect(
    db: &Connection,
    p: &OperationsProfile,
) -> Result<(String, BTreeMap<String, u64>), String> {
    let metadata: (String, String) = db
        .query_row(
            "SELECT schema,service_id FROM hosted_meta WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(err)?;
    if metadata != (SCHEMA.into(), p.service_id.clone()) {
        return Err("incompatible schema or service authority".into());
    }
    let meta_count: i64 = db
        .query_row("SELECT count(*) FROM hosted_meta", [], |r| r.get(0))
        .map_err(err)?;
    if meta_count != 1 {
        return Err("ambiguous service authority".into());
    }
    inspect_sqlite(db)
}
fn inspect_sqlite(db: &Connection) -> Result<(String, BTreeMap<String, u64>), String> {
    db.pragma_update(None, "trusted_schema", false)
        .map_err(err)?;
    let mut stmt = db.prepare("PRAGMA integrity_check(1)").map_err(err)?;
    let integrity: String = stmt.query_row([], |r| r.get(0)).map_err(err)?;
    if integrity != "ok" {
        return Err("SQLite integrity check failed".into());
    }
    let foreign_key_problem = db
        .prepare("PRAGMA foreign_key_check")
        .map_err(err)?
        .query([])
        .map_err(err)?
        .next()
        .map_err(err)?
        .is_some();
    if foreign_key_problem {
        return Err("SQLite foreign key check failed".into());
    }
    let schema_bytes:u64=db.query_row("SELECT coalesce(sum(length(CAST(coalesce(sql,'') AS BLOB))+length(CAST(name AS BLOB))+length(CAST(tbl_name AS BLOB))),0) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",[],|r|crate::sqlite_u64(r,0)).map_err(err)?;
    if schema_bytes > MAX_SCHEMA_BYTES {
        return Err("schema byte bound exceeded".into());
    }
    let mut stmt=db.prepare("SELECT type,name,tbl_name,coalesce(sql,'') FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name LIMIT ?1").map_err(err)?;
    let entries = stmt
        .query_map([(MAX_TABLES * 4 + 1) as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if entries.len() > MAX_TABLES * 4 {
        return Err("schema object bound exceeded".into());
    }
    let schema_hash = crate::identity(&entries)?;
    let mut counts = BTreeMap::new();
    for (kind, name, _, sql) in entries {
        if kind != "table" {
            continue;
        }
        if sql.to_ascii_uppercase().contains("CREATE VIRTUAL TABLE") {
            return Err("virtual tables are not an admitted backup schema".into());
        }
        if counts.len() >= MAX_TABLES {
            return Err("table bound exceeded".into());
        }
        let quoted = name.replace('"', "\"\"");
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM \"{quoted}\""), [], |r| {
                r.get(0)
            })
            .map_err(err)?;
        counts.insert(name, u64::try_from(count).map_err(err)?);
    }
    Ok((schema_hash, counts))
}

fn replay_location(p: &OperationsProfile, path: &Path) -> Result<(OwnedFd, String), String> {
    let parent = path.parent().ok_or("native replay path has no parent")?;
    if !p.replay_roots.iter().any(|r| r == parent) {
        return Err("native replay path requires an explicitly configured replay_root".into());
    }
    let leaf = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("invalid native replay leaf")?;
    if leaf.is_empty()
        || leaf.len() > 240
        || leaf == "."
        || leaf == ".."
        || !leaf
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        return Err("invalid native replay leaf".into());
    }
    Ok((root(parent)?, leaf.into()))
}
fn absent_replay(dir: &OwnedFd, leaf: &str) -> Result<(), String> {
    for name in [
        leaf.to_owned(),
        format!("{leaf}-wal"),
        format!("{leaf}-shm"),
        format!("{leaf}-journal"),
    ] {
        match fs::statat(dir, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {}
            _ => {
                return Err(
                    "native replay target or sidecar exists; fresh recovery roots are required"
                        .into(),
                );
            }
        }
    }
    Ok(())
}
fn snapshot_sqlite(
    db: &Connection,
    dir: &OwnedFd,
    leaf: &str,
    p: &OperationsProfile,
    started: Instant,
    remaining: u64,
) -> Result<(u64, String, String, BTreeMap<String, u64>), String> {
    let size: u64 = db
        .pragma_query_value(None, "page_size", |r| crate::sqlite_u64(r, 0))
        .map_err(err)?;
    let pages: u64 = db
        .pragma_query_value(None, "page_count", |r| crate::sqlite_u64(r, 0))
        .map_err(err)?;
    if pages.checked_mul(size).ok_or("database size overflow")? > remaining {
        return Err("database bundle exceeds backup bound".into());
    }
    create_file(dir, leaf)?.sync_all().map_err(err)?;
    let mut out = Connection::open_with_flags(
        fd_path(dir, leaf),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(err)?;
    {
        let b = rusqlite::backup::Backup::new(db, &mut out).map_err(err)?;
        loop {
            if started.elapsed().as_millis() > u128::from(p.maximum_backup_ms) {
                return Err(
                    "backup bundle deadline exceeded; incomplete snapshot is not restorable".into(),
                );
            }
            let step = b.step(BACKUP_STEP_PAGES).map_err(err)?;
            if u64::try_from(b.progress().pagecount)
                .map_err(err)?
                .checked_mul(size)
                .ok_or("backup size overflow")?
                > remaining
            {
                return Err("growing database exceeds bundle bound".into());
            }
            match step {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => {}
                _ => return Err("backup source busy; incomplete snapshot is not restorable".into()),
            }
        }
    }
    out.pragma_update(None, "journal_mode", "DELETE")
        .map_err(err)?;
    let (schema, counts) = inspect_sqlite(&out)?;
    out.close().map_err(|(_, e)| err(e))?;
    let mut file = open_read(dir, leaf)?;
    file.sync_all().map_err(err)?;
    let (len, digest) = hash_file(&mut file, remaining)?;
    Ok((len, digest, schema, counts))
}
/// Caller MUST exclude all native adapter effects with its service-wide effect barrier.
/// The primary write fence additionally excludes concurrent RPC database commits.
pub fn backup(
    db: &Connection,
    p: &OperationsProfile,
    backup_id: &str,
    now_ms: u64,
) -> Result<BackupManifest, String> {
    validate_profile(p)?;
    name(backup_id)?;
    if !db.is_autocommit() {
        return Err("backup requires a standalone committed service connection".into());
    }
    let started = Instant::now();
    let file_path = db.path().filter(|s| !s.is_empty()).map(PathBuf::from);
    // A separate reader is required: SQLite cannot back up its own active write transaction.
    let _fence = if file_path.is_some() {
        Some(
            rusqlite::Transaction::new_unchecked(db, rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?,
        )
    } else {
        None
    };
    let reader = if let Some(path) = file_path {
        Some(
            Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(err)?,
        )
    } else {
        None
    };
    let primary = reader.as_ref().unwrap_or(db);
    inspect(primary, p)?;
    let has_generations:bool=primary.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='hosted_connection_generations')",[],|r|r.get(0)).map_err(err)?;
    let replay_paths = if has_generations {
        crate::connections::replay_paths(primary)?
    } else {
        Vec::new()
    };
    if replay_paths.len() > MAX_REPLAY_STORES {
        return Err("native replay snapshot inventory exceeds bound".into());
    }
    let root = root(&p.backup_root)?;
    let dir = child(&root, backup_id, true)?;
    let (database_bytes, database_blake3, schema_blake3, table_rows) = snapshot_sqlite(
        primary,
        &dir,
        "state.sqlite3",
        p,
        started,
        p.maximum_database_bytes,
    )?;
    let mut total = database_bytes;
    let mut replay_stores = Vec::new();
    for (index, path) in replay_paths.into_iter().enumerate() {
        let (native_root, leaf) = replay_location(p, &path)?;
        open_read(&native_root, &leaf)?;
        let source = Connection::open_with_flags(
            fd_path(&native_root, &leaf),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(err)?;
        let snapshot_name = format!("source-{index}.sqlite3");
        let (database_bytes, database_blake3, schema_blake3, table_rows) = snapshot_sqlite(
            &source,
            &dir,
            &snapshot_name,
            p,
            started,
            p.maximum_database_bytes
                .checked_sub(total)
                .ok_or("backup bundle overflow")?,
        )?;
        total = total
            .checked_add(database_bytes)
            .ok_or("backup bundle overflow")?;
        replay_stores.push(ReplaySnapshot {
            original_path: path,
            snapshot_name,
            database_bytes,
            database_blake3,
            schema_blake3,
            table_rows,
        });
    }
    let manifest = BackupManifest {
        format: "kiln.hosted.backup.v2".into(),
        service_id: p.service_id.clone(),
        schema: SCHEMA.into(),
        backup_id: backup_id.into(),
        created_at_ms: now_ms,
        database_bytes,
        database_blake3,
        schema_blake3,
        table_rows,
        replay_stores,
    };
    let bytes = serde_json::to_vec(&manifest).map_err(err)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("manifest too large".into());
    }
    let mut f = create_file(&dir, "manifest.json")?;
    f.write_all(&bytes).map_err(err)?;
    f.sync_all().map_err(err)?;
    fs::fsync(&dir).map_err(err)?;
    fs::fsync(&root).map_err(err)?;
    Ok(manifest)
}
struct SnapshotExpected<'a> {
    bytes: u64,
    digest: &'a str,
    schema: &'a str,
    counts: &'a BTreeMap<String, u64>,
}
impl BackupManifest {
    fn expected(&self) -> SnapshotExpected<'_> {
        SnapshotExpected {
            bytes: self.database_bytes,
            digest: &self.database_blake3,
            schema: &self.schema_blake3,
            counts: &self.table_rows,
        }
    }
}
impl ReplaySnapshot {
    fn expected(&self) -> SnapshotExpected<'_> {
        SnapshotExpected {
            bytes: self.database_bytes,
            digest: &self.database_blake3,
            schema: &self.schema_blake3,
            counts: &self.table_rows,
        }
    }
}
fn verify_snapshot(
    dir: &OwnedFd,
    leaf: &str,
    expected: SnapshotExpected<'_>,
    p: &OperationsProfile,
    primary: bool,
) -> Result<(), String> {
    let mut input = open_read(dir, leaf)?;
    let (bytes, digest) = hash_file(&mut input, p.maximum_database_bytes)?;
    if bytes != expected.bytes || digest != expected.digest {
        return Err("backup content digest mismatch".into());
    }
    let db = Connection::open_with_flags(
        fd_path(dir, leaf),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(err)?;
    let inventory = if primary {
        inspect(&db, p)?
    } else {
        inspect_sqlite(&db)?
    };
    if inventory.0 != expected.schema || &inventory.1 != expected.counts {
        return Err("backup schema, authority or obligation inventory differs".into());
    }
    Ok(())
}
fn copy_snapshot(
    source: &OwnedFd,
    source_name: &str,
    target: &OwnedFd,
    target_name: &str,
    expected_bytes: u64,
    expected_digest: &str,
    p: &OperationsProfile,
) -> Result<(), String> {
    let mut input = open_read(source, source_name)?;
    let mut output = create_file(target, target_name)?;
    std::io::copy(
        &mut (&mut input).take(p.maximum_database_bytes + 1),
        &mut output,
    )
    .map_err(err)?;
    output.sync_all().map_err(err)?;
    drop(output);
    let mut copied = open_read(target, target_name)?;
    if hash_file(&mut copied, p.maximum_database_bytes)?
        != (expected_bytes, expected_digest.to_owned())
    {
        return Err("restored content digest mismatch; target remains reserved".into());
    }
    fs::fsync(target).map_err(err)?;
    Ok(())
}
/// Whole-service restore onto a fresh host with the SAME explicitly trusted native replay roots.
/// Existing database/sidecar files are never changed and native profile identities are never rewritten.
pub fn restore(
    p: &OperationsProfile,
    backup_id: &str,
    target_id: &str,
    expected_manifest_identity: &str,
) -> Result<PathBuf, String> {
    validate_profile(p)?;
    name(backup_id)?;
    name(target_id)?;
    let source_root = root(&p.backup_root)?;
    let source = child(&source_root, backup_id, false)?;
    let mf = open_read(&source, "manifest.json")?;
    if mf.metadata().map_err(err)?.len() > MAX_MANIFEST_BYTES {
        return Err("manifest too large".into());
    }
    let mut bytes = Vec::new();
    mf.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    let m: BackupManifest = serde_json::from_slice(&bytes).map_err(err)?;
    if m.identity()? != expected_manifest_identity
        || m.format != "kiln.hosted.backup.v2"
        || m.backup_id != backup_id
        || m.schema != SCHEMA
        || m.service_id != p.service_id
        || m.replay_stores.len() > MAX_REPLAY_STORES
    {
        return Err("untrusted, incompatible or mismatched backup manifest".into());
    }
    let total = m.replay_stores.iter().try_fold(m.database_bytes, |n, s| {
        n.checked_add(s.database_bytes)
            .ok_or("backup bundle overflow")
    })?;
    if total > p.maximum_database_bytes {
        return Err("restore bundle exceeds profile size bound".into());
    }
    verify_snapshot(&source, "state.sqlite3", m.expected(), p, true)?;
    let mut targets = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, s) in m.replay_stores.iter().enumerate() {
        if s.snapshot_name != format!("source-{index}.sqlite3")
            || !seen.insert(s.original_path.clone())
        {
            return Err("invalid or duplicate replay snapshot identity".into());
        }
        let (dir, leaf) = replay_location(p, &s.original_path)?;
        absent_replay(&dir, &leaf)?;
        verify_snapshot(&source, &s.snapshot_name, s.expected(), p, false)?;
        targets.push((dir, leaf));
    }
    let target_root = root(&p.restore_root)?;
    let target = child(&target_root, target_id, true)?;
    // Validate every byte and target before the first native install. Partial failures remain explicit, never overwritten.
    for (s, (dir, leaf)) in m.replay_stores.iter().zip(&targets) {
        absent_replay(dir, leaf)?;
        copy_snapshot(
            &source,
            &s.snapshot_name,
            dir,
            leaf,
            s.database_bytes,
            &s.database_blake3,
            p,
        )?;
        verify_snapshot(dir, leaf, s.expected(), p, false)?;
    }
    // Publish the reopenable primary last, only after every replay store is durable.
    copy_snapshot(
        &source,
        "state.sqlite3",
        &target,
        "hosted.sqlite",
        m.database_bytes,
        &m.database_blake3,
        p,
    )?;
    verify_snapshot(&target, "hosted.sqlite", m.expected(), p, true)?;
    fs::fsync(&target_root).map_err(err)?;
    Ok(p.restore_root.join(target_id).join("hosted.sqlite"))
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_operations_config(singleton INTEGER PRIMARY KEY CHECK(singleton=1),profile TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS hosted_incident_holds(organization TEXT PRIMARY KEY,held INTEGER NOT NULL CHECK(held IN(0,1)),revision INTEGER NOT NULL,reason TEXT NOT NULL,actor TEXT NOT NULL,updated_ms INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS hosted_operations_audit(sequence INTEGER PRIMARY KEY AUTOINCREMENT,organization TEXT NOT NULL,actor TEXT NOT NULL,kind TEXT NOT NULL,subject TEXT NOT NULL,observation TEXT NOT NULL,created_ms INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS hosted_content(organization TEXT NOT NULL,content_id TEXT NOT NULL,storage_id TEXT NOT NULL UNIQUE,digest TEXT NOT NULL,bytes INTEGER NOT NULL,expires_ms INTEGER NOT NULL,state TEXT NOT NULL CHECK(state IN('reserved','available','deleting','deleted')),actor TEXT NOT NULL,created_ms INTEGER NOT NULL,PRIMARY KEY(organization,content_id));
CREATE INDEX IF NOT EXISTS hosted_content_expiry ON hosted_content(organization,state,expires_ms);
CREATE INDEX IF NOT EXISTS hosted_operations_org_audit ON hosted_operations_audit(organization,sequence);").map_err(err)
}
pub fn configure(db: &Connection, p: &OperationsProfile) -> Result<(), String> {
    validate_profile(p)?;
    let metadata: (String, String) = db
        .query_row(
            "SELECT schema,service_id FROM hosted_meta WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(err)?;
    if metadata != (SCHEMA.into(), p.service_id.clone()) {
        return Err("operations profile does not match service authority".into());
    }
    let json = serde_json::to_string(p).map_err(err)?;
    let existing: Option<String> = db
        .query_row(
            "SELECT profile FROM hosted_operations_config WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if let Some(existing) = existing {
        if existing != json {
            return Err("operations roots/profile cannot change in an existing database".into());
        }
        return Ok(());
    }
    db.execute(
        "INSERT INTO hosted_operations_config(singleton,profile) VALUES(1,?1)",
        [json],
    )
    .map_err(err)?;
    Ok(())
}
fn profile(db: &Connection) -> Result<OperationsProfile, String> {
    let p: String = db
        .query_row(
            "SELECT profile FROM hosted_operations_config WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    let p = serde_json::from_str(&p).map_err(err)?;
    validate_profile(&p)?;
    Ok(p)
}
pub fn is_held(db: &Connection, organization: &str) -> Result<bool, String> {
    let held: Option<i64> = db
        .query_row(
            "SELECT held FROM hosted_incident_holds WHERE organization=?1",
            [organization],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    Ok(held == Some(1))
}
fn audit(
    db: &Connection,
    actor: &Actor,
    kind: &str,
    subject: &str,
    observation: &str,
    now: u64,
) -> Result<(), String> {
    db.execute("INSERT INTO hosted_operations_audit(organization,actor,kind,subject,observation,created_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![actor.organization,actor.peer,kind,subject,observation,integer(now)?]).map_err(err)?;
    Ok(())
}

pub fn store_content(
    db: &mut Connection,
    actor: &Actor,
    p: &OperationsProfile,
    content_id: &str,
    bytes: &[u8],
    expires_ms: u64,
    now_ms: u64,
) -> Result<Value, String> {
    crate::require(actor, &["owner", "admin", "member"])?;
    name(content_id)?;
    validate_profile(p)?;
    if bytes.len() as u64 > p.maximum_content_bytes || expires_ms <= now_ms {
        return Err("content size/expiry outside policy".into());
    }
    let digest = blake3::hash(bytes).to_hex().to_string();
    let storage_id = blake3::hash(
        serde_json::to_string(&(&actor.organization, content_id, &digest))
            .map_err(err)?
            .as_bytes(),
    )
    .to_hex()
    .to_string();
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let prior:Option<(String,i64,String)>=tx.query_row("SELECT digest,expires_ms,state FROM hosted_content WHERE organization=?1 AND content_id=?2",params![actor.organization,content_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    if let Some((d, e, state)) = prior {
        if d == digest && e == integer(expires_ms)? && state == "available" {
            return Ok(json!({"content_id":content_id,"digest":digest,"state":state}));
        }
        return Err(
            "content identity already reserved; changed/replayed deletion cannot re-create content"
                .into(),
        );
    }
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM hosted_content WHERE organization=?1 AND state!='deleted'",
            [&actor.organization],
            |r| r.get(0),
        )
        .map_err(err)?;
    if u64::try_from(count).map_err(err)? >= p.maximum_content_records {
        return Err("content inventory limit reached".into());
    }
    tx.execute(
        "INSERT INTO hosted_content VALUES(?1,?2,?3,?4,?5,?6,'reserved',?7,?8)",
        params![
            actor.organization,
            content_id,
            storage_id,
            digest,
            integer(bytes.len() as u64)?,
            integer(expires_ms)?,
            actor.peer,
            integer(now_ms)?
        ],
    )
    .map_err(err)?;
    audit(&tx, actor, "content.reserve", content_id, &digest, now_ms)?;
    tx.commit().map_err(err)?;
    let root = root(&p.content_root)?;
    let mut file = create_file(&root, &storage_id)?;
    file.write_all(bytes).map_err(err)?;
    file.sync_all().map_err(err)?;
    fs::fsync(&root).map_err(err)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    tx.execute("UPDATE hosted_content SET state='available' WHERE organization=?1 AND content_id=?2 AND state='reserved' AND digest=?3",params![actor.organization,content_id,digest]).map_err(err)?;
    audit(&tx, actor, "content.available", content_id, &digest, now_ms)?;
    tx.commit().map_err(err)?;
    Ok(json!({"content_id":content_id,"digest":digest,"state":"available"}))
}
fn sweep(
    db: &mut Connection,
    actor: &Actor,
    p: &OperationsProfile,
    now_ms: u64,
    limit: usize,
) -> Result<Value, String> {
    crate::require(actor, &["owner", "admin"])?;
    if limit == 0 || limit > MAX_LIST {
        return Err("retention limit outside bounds".into());
    }
    let root = root(&p.content_root)?;
    let mut stmt=db.prepare("SELECT content_id,storage_id,digest,bytes FROM hosted_content WHERE organization=?1 AND expires_ms<=?2 AND state IN('available','deleting') ORDER BY expires_ms,content_id LIMIT ?3").map_err(err)?;
    let rows = stmt
        .query_map(
            params![actor.organization, integer(now_ms)?, limit as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    drop(stmt);
    let mut deleted = Vec::new();
    for (id, storage, digest, size) in rows {
        name(&storage)?;
        let expected = blake3::hash(
            serde_json::to_string(&(&actor.organization, &id, &digest))
                .map_err(err)?
                .as_bytes(),
        )
        .to_hex()
        .to_string();
        if storage != expected {
            return Err("retention ownership identity mismatch".into());
        }
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let changed=tx.execute("UPDATE hosted_content SET state='deleting' WHERE organization=?1 AND content_id=?2 AND storage_id=?3 AND digest=?4 AND expires_ms<=?5 AND state IN('available','deleting')",params![actor.organization,id,storage,digest,integer(now_ms)?]).map_err(err)?;
        if changed != 1 {
            return Err("retention authority changed".into());
        }
        audit(&tx, actor, "content.delete.reserve", &id, &digest, now_ms)?;
        tx.commit().map_err(err)?;
        match open_read(&root, &storage) {
            Ok(mut file) => {
                let (len, hash) = hash_file(&mut file, p.maximum_content_bytes)?;
                if len != u64::try_from(size).map_err(err)? || hash != digest {
                    return Err("retention content identity mismatch; nothing deleted".into());
                }
                fs::unlinkat(&root, &storage, AtFlags::empty()).map_err(err)?;
                fs::fsync(&root).map_err(err)?;
            }
            Err(e) => {
                // Only a previously reserved deletion may reconcile an absent exact leaf.
                if fs::statat(&root, &storage, AtFlags::SYMLINK_NOFOLLOW).err()
                    != Some(rustix::io::Errno::NOENT)
                {
                    return Err(e);
                }
            }
        }
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        tx.execute("UPDATE hosted_content SET state='deleted' WHERE organization=?1 AND content_id=?2 AND storage_id=?3 AND state='deleting'",params![actor.organization,id,storage]).map_err(err)?;
        audit(&tx, actor, "content.deleted", &id, &digest, now_ms)?;
        tx.commit().map_err(err)?;
        deleted.push(id);
    }
    Ok(json!({"deleted":deleted,"financial_obligations_released":false}))
}

pub fn handle(
    db: &mut Connection,
    actor: &Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    match action {
        "hold.set" => {
            crate::require(actor, &["owner", "admin"])?;
            check_fields(&input, &["held", "reason", "expected_revision"])?;
            let held = input
                .get("held")
                .and_then(Value::as_bool)
                .ok_or("missing held")?;
            let reason = text(&input, "reason")?;
            let expected = integer(number(&input, "expected_revision")?)?;
            let tx = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            let previous: Option<i64> = tx
                .query_row(
                    "SELECT revision FROM hosted_incident_holds WHERE organization=?1",
                    [&actor.organization],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            if previous.unwrap_or(0) != expected {
                return Err("stale incident hold revision".into());
            }
            let revision = expected.checked_add(1).ok_or("revision exhausted")?;
            tx.execute("INSERT INTO hosted_incident_holds VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(organization) DO UPDATE SET held=excluded.held,revision=excluded.revision,reason=excluded.reason,actor=excluded.actor,updated_ms=excluded.updated_ms",params![actor.organization,i64::from(held),revision,reason,actor.peer,integer(now_ms)?]).map_err(err)?;
            audit(
                &tx,
                actor,
                "incident.hold",
                &revision.to_string(),
                reason,
                now_ms,
            )?;
            tx.commit().map_err(err)?;
            Ok(json!({"held":held,"revision":revision}))
        }
        "hold.get" => {
            crate::require(actor, &["owner", "admin", "member", "viewer", "billing"])?;
            check_fields(&input, &[])?;
            let row:Option<(i64,i64,String,String,i64)>=db.query_row("SELECT held,revision,reason,actor,updated_ms FROM hosted_incident_holds WHERE organization=?1",[&actor.organization],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(err)?;
            Ok(match row {
                Some((h, r, reason, peer, t)) => {
                    json!({"held":h==1,"revision":r,"reason":reason,"actor":peer,"updated_ms":t})
                }
                None => json!({"held":false,"revision":0}),
            })
        }
        "support.record" => {
            crate::require(actor, &["owner", "admin", "member"])?;
            check_fields(&input, &["subject", "observation"])?;
            let subject = text(&input, "subject")?;
            let observation = text(&input, "observation")?;
            audit(
                db,
                actor,
                "support.observation",
                subject,
                observation,
                now_ms,
            )?;
            Ok(json!({"recorded":true,"sequence":db.last_insert_rowid()}))
        }
        "audit.list" => {
            crate::require(actor, &["owner", "admin"])?;
            check_fields(&input, &["after", "limit"])?;
            let after = integer(number(&input, "after")?)?;
            let limit = number(&input, "limit")?;
            if limit == 0 || limit > MAX_LIST as u64 {
                return Err("audit limit outside bounds".into());
            }
            let mut s=db.prepare("SELECT sequence,actor,kind,subject,observation,created_ms FROM hosted_operations_audit WHERE organization=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3").map_err(err)?;
            let rows=s.query_map(params![actor.organization,after,integer(limit)?],|r|Ok(json!({"sequence":r.get::<_,i64>(0)?,"actor":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"subject":r.get::<_,String>(3)?,"observation":r.get::<_,String>(4)?,"created_ms":r.get::<_,i64>(5)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
            Ok(json!({"observations":rows}))
        }
        "health" => {
            crate::require(actor, &["owner", "admin", "member", "viewer", "billing"])?;
            check_fields(&input, &[])?;
            let content:i64=db.query_row("SELECT count(*) FROM hosted_content WHERE organization=?1 AND state='available'",[&actor.organization],|r|r.get(0)).map_err(err)?;
            Ok(
                json!({"scope":"organization-local-observation","organization":actor.organization,"incident_hold":is_held(db,&actor.organization)?,"available_content_records":content,"observed_at_ms":now_ms,"provider_health":"not_observed","availability_claim":false}),
            )
        }
        "content.put" => {
            check_fields(&input, &["content_id", "content_base64", "expires_ms"])?;
            let p = profile(db)?;
            let encoded = input
                .get("content_base64")
                .and_then(Value::as_str)
                .ok_or("missing content_base64")?;
            if encoded.len() as u64 > p.maximum_content_bytes.saturating_mul(4) / 3 + 4 {
                return Err("encoded content exceeds bound".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(err)?;
            store_content(
                db,
                actor,
                &p,
                text(&input, "content_id")?,
                &bytes,
                number(&input, "expires_ms")?,
                now_ms,
            )
        }
        "retention.sweep" => {
            check_fields(&input, &["limit"])?;
            let p = profile(db)?;
            let limit = usize::try_from(number(&input, "limit")?).map_err(err)?;
            sweep(db, actor, &p, now_ms, limit)
        }
        _ => Err("unknown hosted operations action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn actor(org: &str, role: &str) -> Actor {
        Actor {
            peer: "operator-peer".into(),
            organization: org.into(),
            role: role.into(),
        }
    }
    fn fixture() -> (tempfile::TempDir, OperationsProfile, Connection) {
        let tmp =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let roots = ["backup", "restore", "content"].map(|n| {
            let p = tmp.path().join(n);
            std::fs::create_dir(&p)
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
            p
        });
        let p = OperationsProfile {
            service_id: "service-fixture".into(),
            backup_root: roots
                .first()
                .cloned()
                .unwrap_or_else(|| unreachable!("backup root")),
            restore_root: roots
                .get(1)
                .cloned()
                .unwrap_or_else(|| unreachable!("restore root")),
            content_root: roots
                .get(2)
                .cloned()
                .unwrap_or_else(|| unreachable!("content root")),
            replay_roots: vec![],
            maximum_database_bytes: 8 * 1024 * 1024,
            maximum_content_bytes: 1024,
            maximum_content_records: 10,
            maximum_backup_ms: 10_000,
        };
        let db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        db.execute_batch("CREATE TABLE hosted_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema TEXT NOT NULL,service_id TEXT NOT NULL);INSERT INTO hosted_meta VALUES(1,'kiln.hosted.v1','service-fixture');CREATE TABLE accounts(org TEXT PRIMARY KEY,owner TEXT);INSERT INTO accounts VALUES('a','owner-a');CREATE TABLE obligations(attempt TEXT PRIMARY KEY,reserved INTEGER,state TEXT);INSERT INTO obligations VALUES('attempt-1',270,'unknown');CREATE TABLE replay(delivery TEXT PRIMARY KEY,run TEXT);INSERT INTO replay VALUES('delivery-1','run-1');").unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        configure(&db, &p).unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        (tmp, p, db)
    }
    #[test]
    fn whole_restore_preserves_authority_unknown_obligations_and_replay() {
        let (_tmp, p, mut db) = fixture();
        handle(
            &mut db,
            &actor("a", "admin"),
            "hold.set",
            json!({"held":true,"reason":"recovery incident","expected_revision":0}),
            99,
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let manifest = backup(&db, &p, "snapshot", 100)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let path = restore(
            &p,
            "snapshot",
            "recovered",
            &manifest
                .identity()
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let recovered = Connection::open(path)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(
            is_held(&recovered, "a")
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
        );
        assert!(
            !is_held(&recovered, "b")
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
        );
        assert_eq!(
            recovered
                .query_row("SELECT owner FROM accounts WHERE org='a'", [], |r| r
                    .get::<_, String>(0))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            "owner-a"
        );
        assert_eq!(
            recovered
                .query_row(
                    "SELECT reserved,state FROM obligations WHERE attempt='attempt-1'",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                )
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            (270, "unknown".into())
        );
        assert_eq!(
            recovered
                .query_row(
                    "SELECT run FROM replay WHERE delivery='delivery-1'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            "run-1"
        );
        assert!(
            restore(
                &p,
                "snapshot",
                "recovered",
                &manifest
                    .identity()
                    .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
            )
            .is_err()
        );
        assert!(backup(&db, &p, "snapshot", 101).is_err());
    }
    #[test]
    fn bundle_restore_preserves_native_replay_and_refuses_existing_targets() {
        let (tmp, mut p, db) = fixture();
        let native_root = tmp.path().join("native");
        std::fs::create_dir(&native_root)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        std::fs::set_permissions(&native_root, std::fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        p.replay_roots.push(native_root.clone());
        let native_path = native_root.join("hosted-generation.sqlite");
        let native = Connection::open(&native_path)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        native.execute_batch("PRAGMA journal_mode=WAL;CREATE TABLE github_meta(singleton INTEGER PRIMARY KEY,schema TEXT,profile_identity TEXT);INSERT INTO github_meta VALUES(1,'fixture-native-schema','exact-native-profile');CREATE TABLE github_deliveries(delivery_id TEXT PRIMARY KEY,body_identity TEXT,run_identity TEXT);INSERT INTO github_deliveries VALUES('delivery-1','body-1','run-1');").unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let mut m = backup(&db, &p, "bundle", 0)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let backup_root =
            root(&p.backup_root).unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let dir = child(&backup_root, "bundle", false)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let (database_bytes, database_blake3, schema_blake3, table_rows) = snapshot_sqlite(
            &native,
            &dir,
            "source-0.sqlite3",
            &p,
            Instant::now(),
            p.maximum_database_bytes - m.database_bytes,
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        m.replay_stores.push(ReplaySnapshot {
            original_path: native_path.clone(),
            snapshot_name: "source-0.sqlite3".into(),
            database_bytes,
            database_blake3,
            schema_blake3,
            table_rows,
        });
        std::fs::write(
            p.backup_root.join("bundle/manifest.json"),
            serde_json::to_vec(&m)
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let pin = m
            .identity()
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(restore(&p, "bundle", "conflict", &pin).is_err());
        assert!(!p.restore_root.join("conflict").exists());
        drop(native);
        std::fs::remove_file(&native_path)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let restored = restore(&p, "bundle", "recovery", &pin)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert_eq!(
            restored
                .file_name()
                .unwrap_or_else(|| unreachable!("restored file must have a name")),
            "hosted.sqlite"
        );
        let restored_native = Connection::open(&native_path)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert_eq!(
            restored_native
                .query_row("SELECT profile_identity FROM github_meta", [], |r| r
                    .get::<_, String>(0))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            "exact-native-profile"
        );
        assert_eq!(
            restored_native
                .query_row(
                    "SELECT run_identity FROM github_deliveries WHERE delivery_id='delivery-1'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            "run-1"
        );
        assert!(restore(&p, "bundle", "second", &pin).is_err());
        assert!(!p.restore_root.join("second").exists());
    }
    #[test]
    fn file_backed_backup_fences_writers_while_snapshotting_committed_state() {
        let (tmp, p, db) = fixture();
        let primary_path = tmp.path().join("primary.sqlite");
        db.backup(rusqlite::MAIN_DB, &primary_path, None)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let primary = Connection::open(primary_path)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        primary
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let m = backup(&primary, &p, "fenced", 0)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(primary.is_autocommit());
        let restored = restore(
            &p,
            "fenced",
            "recovery",
            &m.identity()
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert_eq!(
            Connection::open(restored)
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
                .query_row("SELECT reserved FROM obligations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            270
        );
    }
    #[test]
    fn restore_rejects_corruption_manifest_tampering_and_wrong_service() {
        let (_tmp, p, db) = fixture();
        let m = backup(&db, &p, "snapshot", 0)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let pin = m
            .identity()
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let mut wrong = p.clone();
        wrong.service_id = "other-service".into();
        assert!(restore(&wrong, "snapshot", "wrong", &pin).is_err());
        assert!(restore(&p, "snapshot", "untrusted", "wrong-hash").is_err());
        let path = p.backup_root.join("snapshot/state.sqlite3");
        let mut data =
            std::fs::read(&path).unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        *data
            .first_mut()
            .unwrap_or_else(|| unreachable!("snapshot nonempty")) ^= 1;
        std::fs::write(path, data)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(restore(&p, "snapshot", "corrupt", &pin).is_err());
        assert!(!p.restore_root.join("corrupt").exists());
    }
    #[test]
    fn held_scope_revision_and_audit_are_durable_and_attributable() {
        let (_tmp, _p, mut db) = fixture();
        let a = actor("a", "admin");
        let b = actor("b", "admin");
        assert!(
            handle(
                &mut db,
                &actor("a", "viewer"),
                "hold.set",
                json!({"held":true,"reason":"incident","expected_revision":0}),
                100
            )
            .is_err()
        );
        handle(
            &mut db,
            &a,
            "hold.set",
            json!({"held":true,"reason":"incident","expected_revision":0}),
            100,
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(
            is_held(&db, "a").unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
        );
        assert!(
            !is_held(&db, "b").unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
        );
        assert!(
            handle(
                &mut db,
                &a,
                "hold.set",
                json!({"held":false,"reason":"clear","expected_revision":0}),
                101
            )
            .is_err()
        );
        let audit = handle(
            &mut db,
            &a,
            "audit.list",
            json!({"after":0,"limit":10}),
            102,
        )
        .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert_eq!(
            audit
                .pointer("/observations/0/actor")
                .cloned()
                .unwrap_or(Value::Null),
            a.peer
        );
        assert_eq!(
            audit
                .pointer("/observations/0/kind")
                .cloned()
                .unwrap_or(Value::Null),
            "incident.hold"
        );
        assert_eq!(
            handle(
                &mut db,
                &b,
                "audit.list",
                json!({"after":0,"limit":10}),
                102
            )
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"))
            .get("observations")
            .cloned()
            .unwrap_or(Value::Null),
            json!([])
        );
    }
    #[test]
    fn retention_deletes_only_expired_owned_content_and_never_obligations() {
        let (_tmp, p, mut db) = fixture();
        let a = actor("a", "admin");
        let b = actor("b", "admin");
        store_content(&mut db, &a, &p, "expired", b"old", 100, 1)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        store_content(&mut db, &a, &p, "future", b"new", 200, 1)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        store_content(&mut db, &b, &p, "expired", b"other", 100, 1)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let out = sweep(&mut db, &a, &p, 100, 10)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert_eq!(
            out.get("deleted").cloned().unwrap_or(Value::Null),
            json!(["expired"])
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM hosted_content WHERE state='available'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            2
        );
        assert_eq!(
            db.query_row("SELECT reserved FROM obligations", [], |r| r
                .get::<_, i64>(0))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            270
        );
        assert_eq!(
            db.query_row("SELECT run FROM replay", [], |r| r.get::<_, String>(0))
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            "run-1"
        );
        assert!(store_content(&mut db, &a, &p, "expired", b"old", 100, 1).is_err());
        assert!(store_content(&mut db, &a, &p, "../unsafe", b"bad", 200, 1).is_err());
        assert!(sweep(&mut db, &actor("a", "member"), &p, 200, 10).is_err());
    }
    #[test]
    fn traversal_symlinks_and_replaced_content_fail_closed() {
        let (_tmp, p, mut db) = fixture();
        assert!(backup(&db, &p, "../outside", 0).is_err());
        std::os::unix::fs::symlink(&p.content_root, p.backup_root.join("alias"))
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(backup(&db, &p, "alias", 0).is_err());
        let a = actor("a", "admin");
        store_content(&mut db, &a, &p, "expired", b"owned", 100, 1)
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let storage: String = db
            .query_row("SELECT storage_id FROM hosted_content", [], |r| r.get(0))
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        let outside = p.restore_root.join("important");
        std::fs::write(&outside, b"protected")
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        std::fs::remove_file(p.content_root.join(&storage))
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        std::os::unix::fs::symlink(&outside, p.content_root.join(&storage))
            .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}"));
        assert!(sweep(&mut db, &a, &p, 100, 10).is_err());
        assert_eq!(
            std::fs::read(outside)
                .unwrap_or_else(|error| unreachable!("fixture failure: {error:?}")),
            b"protected"
        );
    }
}
