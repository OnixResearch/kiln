use std::path::{Path, PathBuf};

use kiln_codec::identity_from_bytes;
use kiln_core::source::PijulSnapshotLimits;
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PijulReadObservation, PijulReadRequest, PijulRepositoryPort,
    PortFailure, PortResult,
};
use libpijul::{Base32, TxnT, TxnTExt};
use pijul_repository::Repository;

use crate::archive::SnapshotArchive;
use crate::profile::{PINNED_FORMAT_COHORT, PijulSourceProfile, admit_profile};

const PIJUL_DOT_DIRECTORY: &str = ".pijul";
const PIJUL_PRISTINE_DIRECTORY: &str = "pristine";
const PIJUL_PRISTINE_DATABASE: &str = "db";
const PIJUL_CHANGES_DIRECTORY: &str = "changes";

#[derive(Debug)]
pub struct LocalPijulAdapter {
    repository_path: PathBuf,
    channel: String,
    profile: PijulSourceProfile,
}

impl LocalPijulAdapter {
    pub fn new(
        repository_path: PathBuf,
        channel: String,
        profile: PijulSourceProfile,
    ) -> PortResult<Self> {
        let _admitted = admit_profile(&profile)?;
        if channel.is_empty() || channel.len() > profile.limits.max_path_bytes {
            return Err(Diagnostic::new(
                "pijul_channel_bound",
                "pijul.channel",
                "Pijul channel is empty or exceeds its named bound",
            )
            .into());
        }
        Ok(Self {
            repository_path,
            channel,
            profile,
        })
    }

    fn validate_layout(&self) -> PortResult<()> {
        let dot = self.repository_path.join(PIJUL_DOT_DIRECTORY);
        if !dot.is_dir() {
            return Err(Diagnostic::new(
                "pijul_repository_missing",
                "pijul.repository",
                "Explicit local path is not a Pijul repository",
            )
            .into());
        }
        let pristine = dot
            .join(PIJUL_PRISTINE_DIRECTORY)
            .join(PIJUL_PRISTINE_DATABASE);
        let changes = dot.join(PIJUL_CHANGES_DIRECTORY);
        if !pristine.is_file() || !changes.is_dir() {
            return Err(PortFailure::Infrastructure(InfrastructureFailure {
                code: "pijul_repository_damaged".to_string(),
                message: "Pijul repository is missing its pristine database or change store"
                    .to_string(),
            }));
        }
        self.admit_repository_storage(&dot)
    }

    fn admit_repository_storage(&self, dot_directory: &Path) -> PortResult<()> {
        let mut pending = vec![dot_directory.to_path_buf()];
        let mut members = 0usize;
        let mut total_bytes = 0u64;
        while let Some(directory) = pending.pop() {
            let entries = std::fs::read_dir(&directory).map_err(io_failure)?;
            for entry in entries {
                let entry = entry.map_err(io_failure)?;
                if members >= self.profile.max_repository_members {
                    return Err(Diagnostic::new(
                        "pijul_repository_member_bound",
                        "pijul.repository",
                        "Pijul repository member count exceeds its named bound",
                    )
                    .into());
                }
                members = members.checked_add(1).ok_or_else(|| {
                    PortFailure::Domain(Diagnostic::new(
                        "pijul_repository_member_bound",
                        "pijul.repository",
                        "Pijul repository member count overflowed",
                    ))
                })?;
                let path = entry.path();
                let relative = path.strip_prefix(dot_directory).map_err(|_| {
                    PortFailure::Domain(Diagnostic::new(
                        "pijul_repository_path",
                        "pijul.repository",
                        "Pijul repository member escaped its explicit root",
                    ))
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    PortFailure::Domain(Diagnostic::new(
                        "pijul_repository_path",
                        "pijul.repository",
                        "Pijul repository member path is not valid UTF-8",
                    ))
                })?;
                if relative.len() > self.profile.limits.max_path_bytes {
                    return Err(Diagnostic::new(
                        "pijul_repository_path_bound",
                        "pijul.repository",
                        "Pijul repository member path exceeds its named bound",
                    )
                    .into());
                }
                let metadata = std::fs::symlink_metadata(&path).map_err(io_failure)?;
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    return Err(Diagnostic::new(
                        "pijul_repository_entry_kind",
                        "pijul.repository",
                        "Pijul repository metadata contains an unsupported symbolic link",
                    )
                    .into());
                }
                if file_type.is_dir() {
                    pending.push(path);
                } else if file_type.is_file() {
                    total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
                        PortFailure::Domain(Diagnostic::new(
                            "pijul_repository_byte_bound",
                            "pijul.repository",
                            "Pijul repository byte count overflowed",
                        ))
                    })?;
                    if total_bytes > self.profile.max_repository_bytes {
                        return Err(Diagnostic::new(
                            "pijul_repository_byte_bound",
                            "pijul.repository",
                            "Pijul repository bytes exceed their named bound",
                        )
                        .into());
                    }
                } else {
                    return Err(Diagnostic::new(
                        "pijul_repository_entry_kind",
                        "pijul.repository",
                        "Pijul repository metadata contains an unsupported entry kind",
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    fn open(&self) -> PortResult<Repository> {
        self.validate_layout()?;
        let expected = self.repository_path.canonicalize().map_err(io_failure)?;
        let repository = Repository::find_root(Some(&self.repository_path))
            .map_err(|error| repository_failure(&error.to_string()))?;
        let observed = repository.path.canonicalize().map_err(io_failure)?;
        if observed != expected {
            return Err(Diagnostic::new(
                "pijul_repository_substitution",
                "pijul.repository",
                "Pijul repository discovery moved outside the explicit root",
            )
            .into());
        }
        Ok(repository)
    }

    fn repository_identity(repository: &Repository) -> PortResult<ExternalRef> {
        let canonical = repository.path.canonicalize().map_err(io_failure)?;
        let bytes = path_bytes(&canonical)?;
        let identity = identity_from_bytes(bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "pijul_repository_identity",
                "pijul.repository",
                "Pijul repository identity could not be computed",
            ))
        })?;
        ExternalRef::new("pijul", "repository", identity.as_str()).map_err(PortFailure::Domain)
    }

    fn validate_request(&self, request: &PijulReadRequest) -> PortResult<()> {
        if request.channel.provider_namespace.as_str() != "pijul"
            || request.channel.native_kind.as_str() != "channel"
            || request.channel.native_id.as_str() != self.channel
            || request.limits != self.profile.limits
        {
            return Err(Diagnostic::new(
                "pijul_request_substitution",
                "pijul.request",
                "Pijul read request differs from the configured channel or bounds",
            )
            .into());
        }
        Ok(())
    }

    fn collect_state(
        &self,
        repository: &Repository,
        limits: &PijulSnapshotLimits,
    ) -> PortResult<NativeRead> {
        let transaction = repository
            .pristine
            .arc_txn_begin()
            .map_err(|error| repository_failure(&error.to_string()))?;
        let channel = {
            let read = transaction.read();
            read.load_channel(&self.channel)
                .map_err(|error| repository_failure(&error.to_string()))?
                .ok_or_else(|| {
                    PortFailure::Domain(Diagnostic::new(
                        "pijul_channel_missing",
                        "pijul.channel",
                        "Configured Pijul channel does not exist",
                    ))
                })?
        };
        let (state_before, change_sequence) = {
            let read = transaction.read();
            let channel_read = channel.read();
            let state = read
                .current_state(&channel_read)
                .map_err(|error| repository_failure(&error.to_string()))?;
            let mut changes = Vec::new();
            let log = read
                .log(&channel_read, 0)
                .map_err(|error| repository_failure(&error.to_string()))?;
            for record in log {
                if changes.len() >= limits.max_changes {
                    return Err(Diagnostic::new(
                        "pijul_change_bound",
                        "pijul.change_sequence",
                        "Pijul change sequence exceeds its named bound",
                    )
                    .into());
                }
                let (_timestamp, (hash, _state)) =
                    record.map_err(|error| repository_failure(&error.to_string()))?;
                let hash: libpijul::Hash = hash.into();
                changes.push(ExternalRef::new("pijul", "change", &hash.to_base32())?);
            }
            (state, changes)
        };
        let mut archive = SnapshotArchive::new(limits.clone());
        let conflicts = transaction
            .archive::<_, _, libpijul::working_copy::filesystem::FileSystem>(
                &repository.changes,
                &channel,
                &mut archive,
            )
            .map_err(|error| archive_failure(&error.to_string()))?;
        if !conflicts.is_empty() {
            return Err(Diagnostic::new(
                "pijul_snapshot_conflict",
                "pijul.snapshot",
                "Pijul channel contains conflicts outside this snapshot profile",
            )
            .into());
        }
        let state_after = {
            let read = transaction.read();
            let channel_read = channel.read();
            read.current_state(&channel_read)
                .map_err(|error| repository_failure(&error.to_string()))?
        };
        Ok(NativeRead {
            state_before: state_before.to_base32(),
            state_after: state_after.to_base32(),
            change_sequence,
            entries: archive.into_entries(),
        })
    }
}

#[derive(Debug)]
struct NativeRead {
    state_before: String,
    state_after: String,
    change_sequence: Vec<ExternalRef>,
    entries: Vec<kiln_core::source::PijulSnapshotEntry>,
}

impl PijulRepositoryPort for LocalPijulAdapter {
    fn discover_channels(&self, max_channels: usize) -> PortResult<Vec<ExternalRef>> {
        if max_channels == 0 {
            return Err(Diagnostic::new(
                "pijul_channel_bound",
                "pijul.channels",
                "Pijul channel discovery requires a positive bound",
            )
            .into());
        }
        let repository = self.open()?;
        let transaction = repository
            .pristine
            .txn_begin()
            .map_err(|error| repository_failure(&error.to_string()))?;
        let found = transaction
            .load_channel(&self.channel)
            .map_err(|error| repository_failure(&error.to_string()))?
            .is_some();
        if !found {
            return Err(Diagnostic::new(
                "pijul_channel_missing",
                "pijul.channel",
                "Configured Pijul channel does not exist",
            )
            .into());
        }
        Ok(vec![ExternalRef::new("pijul", "channel", &self.channel)?])
    }

    fn resolve_and_acquire(&self, request: &PijulReadRequest) -> PortResult<PijulReadObservation> {
        self.validate_request(request)?;
        let repository = self.open()?;
        let repository_identity = Self::repository_identity(&repository)?;
        let read = self.collect_state(&repository, &request.limits)?;
        self.admit_repository_storage(&self.repository_path.join(PIJUL_DOT_DIRECTORY))?;
        Ok(PijulReadObservation {
            repository: repository_identity,
            channel: request.channel.clone(),
            observed_state_before: ExternalRef::new("pijul", "state", &read.state_before)?,
            observed_state_after: ExternalRef::new("pijul", "state", &read.state_after)?,
            change_sequence: read.change_sequence,
            format_cohort: ExternalRef::new("pijul", "format-cohort", PINNED_FORMAT_COHORT)?,
            entries: read.entries,
        })
    }
}

fn path_bytes(path: &Path) -> PortResult<&[u8]> {
    path.to_str().map(str::as_bytes).ok_or_else(|| {
        PortFailure::Domain(Diagnostic::new(
            "pijul_repository_path",
            "pijul.repository",
            "Pijul repository path is not valid UTF-8",
        ))
    })
}

fn repository_failure(message: &str) -> PortFailure {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("locked") || normalized.contains("would block") {
        return PortFailure::Retryable(InfrastructureFailure {
            code: "pijul_repository_busy".to_string(),
            message: "Pijul repository is busy".to_string(),
        });
    }
    if normalized.contains("version mismatch") {
        return PortFailure::Domain(Diagnostic::new(
            "pijul_format_unsupported",
            "pijul.repository_format",
            "Pijul pristine format differs from the selected cohort",
        ));
    }
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "pijul_repository_damaged".to_string(),
        message: "Pijul repository could not provide a complete native observation".to_string(),
    })
}

fn archive_failure(_message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "pijul_snapshot_provider_error".to_string(),
        message: "Pijul change store could not produce the exact channel snapshot".to_string(),
    })
}

fn io_failure(_error: std::io::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "pijul_repository_io".to_string(),
        message: "Pijul repository path could not be observed".to_string(),
    })
}
