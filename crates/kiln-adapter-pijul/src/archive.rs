use std::fmt::{Display, Formatter};
use std::io::{self, Write};

use kiln_core::source::{PijulEntryKind, PijulSnapshotEntry, PijulSnapshotLimits};
use libpijul::output::Archive;

const UNIX_EXECUTABLE_MASK: u16 = 0o111;

#[derive(Debug)]
pub struct SnapshotArchiveError {
    message: &'static str,
}

impl SnapshotArchiveError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for SnapshotArchiveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for SnapshotArchiveError {}

#[derive(Debug)]
pub struct SnapshotFile {
    path: Option<String>,
    kind: PijulEntryKind,
    bytes: Vec<u8>,
    max_file_bytes: usize,
}

impl Write for SnapshotFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.path.is_none() {
            return Err(io::Error::other(
                "Pijul archive path exceeds its named bound",
            ));
        }
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("Pijul archive file byte count overflowed"))?;
        if next > self.max_file_bytes {
            return Err(io::Error::other(
                "Pijul archive file exceeds its named byte bound",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct SnapshotArchive {
    entries: Vec<PijulSnapshotEntry>,
    limits: PijulSnapshotLimits,
    total_bytes: usize,
}

impl SnapshotArchive {
    #[must_use]
    pub const fn new(limits: PijulSnapshotLimits) -> Self {
        Self {
            entries: Vec::new(),
            limits,
            total_bytes: 0,
        }
    }

    #[must_use]
    pub fn into_entries(self) -> Vec<PijulSnapshotEntry> {
        self.entries
    }

    fn admit_entry(&self) -> Result<(), SnapshotArchiveError> {
        if self.entries.len() >= self.limits.max_entries {
            return Err(SnapshotArchiveError::new(
                "Pijul archive entry count exceeds its named bound",
            ));
        }
        Ok(())
    }

    fn bounded_path(&self, path: &str) -> Option<String> {
        (path.len() <= self.limits.max_path_bytes).then(|| path.to_string())
    }
}

impl Archive for SnapshotArchive {
    type File = SnapshotFile;
    type Error = SnapshotArchiveError;

    fn create_file(&mut self, path: &str, _mtime: u64, permissions: u16) -> Self::File {
        SnapshotFile {
            path: self.bounded_path(path),
            kind: if permissions & UNIX_EXECUTABLE_MASK == 0 {
                PijulEntryKind::Regular
            } else {
                PijulEntryKind::Executable
            },
            bytes: Vec::new(),
            max_file_bytes: self.limits.max_file_bytes,
        }
    }

    fn create_dir(
        &mut self,
        path: &str,
        _mtime: u64,
        _permissions: u16,
    ) -> Result<(), Self::Error> {
        self.admit_entry()?;
        let bounded_path = self.bounded_path(path).ok_or_else(|| {
            SnapshotArchiveError::new("Pijul archive path exceeds its named bound")
        })?;
        self.entries.push(PijulSnapshotEntry {
            path: bounded_path,
            kind: PijulEntryKind::Directory,
            bytes: Vec::new(),
        });
        Ok(())
    }

    fn close_file(&mut self, file: Self::File) -> Result<(), Self::Error> {
        self.admit_entry()?;
        let path = file.path.ok_or_else(|| {
            SnapshotArchiveError::new("Pijul archive path exceeds its named bound")
        })?;
        self.total_bytes = self
            .total_bytes
            .checked_add(file.bytes.len())
            .ok_or_else(|| {
                SnapshotArchiveError::new("Pijul archive total byte count overflowed")
            })?;
        if self.total_bytes > self.limits.max_total_bytes {
            return Err(SnapshotArchiveError::new(
                "Pijul archive exceeds its named total byte bound",
            ));
        }
        self.entries.push(PijulSnapshotEntry {
            path,
            kind: file.kind,
            bytes: file.bytes,
        });
        Ok(())
    }
}
