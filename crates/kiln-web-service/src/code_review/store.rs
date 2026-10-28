//! Private SQLite document and request journal. A separate advisory lock spans
//! reservation, provider effect and reconciliation across cooperating processes.
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

const MAX_STATE: usize = 16 * 1024 * 1024;
const MAX_REQUESTS: i64 = 20_000;
const MAX_DATABASE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DATABASE_PAGES: u32 = 32768;
const DATABASE_BUSY_WINDOW_MS: u64 = 750;

#[derive(Debug)]
pub(super) struct Store {
    connection: Connection,
    _lock: FileLock,
}

#[derive(Debug)]
struct FileLock(File);

impl Drop for FileLock {
    fn drop(&mut self) {
        // Closing our fd alone leaves flock held by a concurrent fork's copy.
        // Store drops its SQLite connection before this guard releases admission.
        let _ = FileExt::unlock(&self.0);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Journal {
    pub principal: String,
    pub request_id: String,
    pub digest: String,
    pub pending: Option<serde_json::Value>,
    pub response: Option<serde_json::Value>,
}

fn private_file(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "review private file unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "review file metadata unavailable")?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err("review file must be owned private regular file".into());
    }
    Ok(file)
}

pub(super) fn preflight(path: &Path) -> Result<(), String> {
    if !crate::profile::absolute(path) {
        return Err("review database path invalid".into());
    }
    let parent = path.parent().ok_or("review database parent missing")?;
    crate::worker::files::owned_directory(parent)?;
    for ancestor in parent.ancestors() {
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|_| "review ancestor unavailable")?;
        if metadata.file_type().is_symlink()
            || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
        {
            return Err("review ancestors permit replacement".into());
        }
    }
    for candidate in [
        path.to_path_buf(),
        path.with_extension("review-lock"),
        Path::new(&format!("{}-journal", path.display())).to_path_buf(),
    ] {
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if !metadata.is_file()
                    || metadata.nlink() != 1
                    || metadata.uid() != nix::unistd::geteuid().as_raw()
                    || metadata.mode() & 0o777 != 0o600
                    || metadata.len() > MAX_DATABASE_BYTES
                {
                    return Err("review database or sidecar unsafe".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("review file admission unavailable".into()),
        }
    }
    Ok(())
}

impl Store {
    pub fn open(path: &Path, binding: &str) -> Result<Self, String> {
        preflight(path)?;
        let lock = private_file(&path.with_extension("review-lock"))?;
        lock.try_lock_exclusive()
            .map_err(|_| "review transaction busy")?;
        let lock = FileLock(lock);
        let _file = private_file(path)?;
        let connection = Connection::open(path).map_err(|_| "review database unavailable")?;
        connection
            .busy_timeout(std::time::Duration::from_millis(DATABASE_BUSY_WINDOW_MS))
            .map_err(|_| "review timeout unavailable")?;
        connection
            .pragma_update(None, "max_page_count", MAX_DATABASE_PAGES)
            .map_err(|_| "review database bound unavailable")?;
        connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS review_state(id INTEGER PRIMARY KEY CHECK(id=1), binding TEXT NOT NULL, document BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS review_requests(principal TEXT NOT NULL, request_id TEXT NOT NULL, pending INTEGER NOT NULL CHECK(pending IN(0,1)), document BLOB NOT NULL, PRIMARY KEY(principal,request_id));
            CREATE INDEX IF NOT EXISTS review_pending ON review_requests(pending) WHERE pending=1;")
            .map_err(|_| "review schema unavailable")?;
        let existing: Option<String> = connection
            .query_row("SELECT binding FROM review_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| "review binding unavailable")?;
        if existing.as_deref().is_some_and(|old| old != binding) {
            return Err("review profile binding changed".into());
        }
        Ok(Self {
            connection,
            _lock: lock,
        })
    }

    pub fn load<T: serde::de::DeserializeOwned + Default>(&self) -> Result<T, String> {
        let length: Option<i64> = self
            .connection
            .query_row(
                "SELECT length(document) FROM review_state WHERE id=1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| "review state length unavailable")?;
        if length.is_some_and(|length| length < 0 || length as u64 > MAX_STATE as u64) {
            return Err("review state exceeds bounds".into());
        }
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row("SELECT document FROM review_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| "review state unavailable")?;
        match bytes {
            None => Ok(T::default()),
            Some(bytes) if bytes.len() <= MAX_STATE => {
                serde_json::from_slice(&bytes).map_err(|_| "review state malformed".into())
            }
            _ => Err("review state exceeds bounds".into()),
        }
    }

    pub fn request(&self, principal: &str, request_id: &str) -> Result<Option<Journal>, String> {
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT document FROM review_requests WHERE principal=?1 AND request_id=?2",
                params![principal, request_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| "review request unavailable")?;
        bytes
            .map(|bytes| {
                serde_json::from_slice(&bytes).map_err(|_| "review request malformed".into())
            })
            .transpose()
    }

    pub fn pending(&self) -> Result<Vec<Journal>, String> {
        let mut query = self.connection.prepare("SELECT document FROM review_requests WHERE pending=1 ORDER BY principal,request_id LIMIT ?1").map_err(|_| "review recovery unavailable")?;
        let rows = query
            .query_map([super::MAX_OPERATIONS as i64 + 1], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(|_| "review recovery unavailable")?;
        let mut pending = Vec::new();
        for row in rows {
            let bytes = row.map_err(|_| "review recovery row unavailable")?;
            let journal: Journal =
                serde_json::from_slice(&bytes).map_err(|_| "review recovery malformed")?;
            if journal.pending.is_some() {
                pending.push(journal);
            }
            if pending.len() > super::MAX_OPERATIONS {
                return Err("review recovery exceeds bounds".into());
            }
        }
        Ok(pending)
    }

    pub fn commit<T: Serialize>(
        &mut self,
        binding: &str,
        state: &T,
        journal: Option<&Journal>,
    ) -> Result<(), String> {
        let bytes = serde_json::to_vec(state).map_err(|_| "review state encoding failed")?;
        if bytes.len() > MAX_STATE {
            return Err("review state exceeds bound".into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "review transaction unavailable")?;
        if let Some(journal) = journal {
            let count: i64 = transaction
                .query_row("SELECT count(*) FROM review_requests", [], |row| row.get(0))
                .map_err(|_| "review request count unavailable")?;
            let exists: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM review_requests WHERE principal=?1 AND request_id=?2)", params![journal.principal,journal.request_id], |row| row.get(0)).map_err(|_| "review request count unavailable")?;
            if count >= MAX_REQUESTS && !exists {
                return Err("review request journal full".into());
            }
            let entry =
                serde_json::to_vec(journal).map_err(|_| "review request encoding failed")?;
            transaction.execute("INSERT INTO review_requests(principal,request_id,pending,document) VALUES(?1,?2,?3,?4) ON CONFLICT(principal,request_id) DO UPDATE SET pending=excluded.pending,document=excluded.document", params![journal.principal,journal.request_id,journal.pending.is_some(),entry]).map_err(|_| "review request commit failed")?;
        }
        transaction.execute("INSERT INTO review_state(id,binding,document) VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET document=excluded.document", params![binding,bytes]).map_err(|_| "review state commit failed")?;
        transaction
            .commit()
            .map_err(|_| "review durable commit failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn completed_store_releases_lock_while_inherited_descriptor_remains_open()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = directory.path().join("review.sqlite");
        let store = Store::open(&path, "fixture")?;
        assert!(Store::open(&path, "fixture").is_err());
        // dup and fork retain the same open file description and flock ownership.
        let inherited = store._lock.0.try_clone()?;
        drop(store);
        let reopened = Store::open(&path, "fixture");
        assert!(
            reopened.is_ok(),
            "completed store retained its lock: {reopened:?}"
        );
        drop(inherited);
        Ok(())
    }
}
