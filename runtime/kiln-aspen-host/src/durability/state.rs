use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use durable_file_publication::core::{
    DurabilityMode, PublicationDisposition, PublicationRequest, ReplacementMode,
};
use durable_file_publication::shell::{LinuxDirectory, StageNameSource, publish_one_file};
use molten::system_extension::NATIVE_INSTANCE_STATE_SCHEMA;

use super::HostStateMode;
use super::model::{DURABLE_HOST_STATE_SCHEMA, DURABLE_OPERATION_SCHEMA, DurableHostStateMarker};
use crate::error::{HostBridgeError, Result};

pub(super) const LEDGER_DIRECTORY: &str = "operation-ledger";
pub(super) const LEDGER_FILE: &str = "operations.sqlite";
pub(super) const NATIVE_JOURNAL_DIRECTORY: &str = "native-journal";
pub(super) const NATIVE_JOURNAL_FILE: &str = "fabric-durability.redb";
pub(super) const CALLBACK_VALUE_DIRECTORY: &str = "callback-values";
pub(super) const STATE_MARKER_FILE: &str = "host-state.json";
const CALLBACK_VALUE_SCHEMA: &str = "kiln.aspen-host.callback-values.v1";
const STATE_DIRECTORY_MODE: u32 = 0o700;
const STATE_MARKER_MODE: u32 = 0o600;
const STATE_MARKER_MAXIMUM_BYTES: usize = 4_096;
const STATE_MARKER_OVERFLOW_SENTINEL_BYTES: u64 = 1;
const STATE_MARKER_STAGE_ATTEMPTS: usize = 8;

#[derive(Debug)]
struct MarkerStageNames {
    cohort_identity: String,
    next: usize,
}

impl StageNameSource for MarkerStageNames {
    fn next_stage_leaf(&mut self) -> String {
        let selected = self.next;
        self.next = self.next.saturating_add(1);
        format!("stage-{}-{selected}", self.cohort_identity)
    }
}

pub(super) fn expected_marker(cohort_identity: &str) -> DurableHostStateMarker {
    DurableHostStateMarker {
        schema: DURABLE_HOST_STATE_SCHEMA.to_string(),
        cohort_identity: cohort_identity.to_string(),
        operation_schema: DURABLE_OPERATION_SCHEMA.to_string(),
        native_journal_schema: NATIVE_INSTANCE_STATE_SCHEMA.to_string(),
        callback_value_schema: CALLBACK_VALUE_SCHEMA.to_string(),
    }
}

pub(super) fn prepare_directory(root: &Path, leaf: &str, mode: HostStateMode) -> Result<PathBuf> {
    let path = root.join(leaf);
    match mode {
        HostStateMode::Fresh => prepare_fresh_directory(&path, leaf)?,
        HostStateMode::Recovered => require_directory(&path, leaf)?,
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(STATE_DIRECTORY_MODE))
        .map_err(|error| {
            HostBridgeError::Io(format!("set state path {leaf} permissions: {error}"))
        })?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| HostBridgeError::Io(format!("synchronize host state root: {error}")))?;
    Ok(path)
}

pub(super) fn publish_marker(root: &Path, marker: &DurableHostStateMarker) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(marker)
        .map_err(|_| HostBridgeError::State("encode host state marker".to_string()))?;
    if bytes.len() > STATE_MARKER_MAXIMUM_BYTES {
        return Err(HostBridgeError::State(
            "host state marker exceeds its byte bound".to_string(),
        ));
    }
    let directory = File::open(root)
        .map_err(|error| HostBridgeError::Io(format!("open host state root: {error}")))?;
    let mut publication = LinuxDirectory::from_open_directory(directory)
        .map_err(|error| HostBridgeError::Io(format!("admit host state root: {error}")))?;
    let request = PublicationRequest {
        destination_leaf: STATE_MARKER_FILE.to_string(),
        payload_bytes: bytes.len(),
        maximum_payload_bytes: STATE_MARKER_MAXIMUM_BYTES,
        collision_attempt_limit: STATE_MARKER_STAGE_ATTEMPTS,
        final_mode: STATE_MARKER_MODE,
        replacement: ReplacementMode::NoReplace,
        durability: DurabilityMode::DurabilityRequired,
    };
    let mut names = MarkerStageNames {
        cohort_identity: blake3::hash(marker.cohort_identity.as_bytes())
            .to_hex()
            .to_string(),
        next: 0,
    };
    let classification = publish_one_file(&mut publication, &mut names, &request, &bytes)
        .map_err(|error| HostBridgeError::Io(format!("publish host state marker: {error:?}")))?;
    if classification.disposition != PublicationDisposition::CommittedAndParentSynchronized {
        return Err(HostBridgeError::State(
            "host state marker did not reach durable publication".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn load_marker(path: &Path) -> Result<DurableHostStateMarker> {
    require_regular_file(path, "host state marker")?;
    let file = File::open(path)
        .map_err(|error| HostBridgeError::Io(format!("open host state marker: {error}")))?;
    let read_limit = u64::try_from(STATE_MARKER_MAXIMUM_BYTES)
        .ok()
        .and_then(|value| value.checked_add(STATE_MARKER_OVERFLOW_SENTINEL_BYTES))
        .ok_or_else(|| HostBridgeError::State("host state marker bound overflow".to_string()))?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| HostBridgeError::Io(format!("read host state marker: {error}")))?;
    if bytes.len() > STATE_MARKER_MAXIMUM_BYTES {
        return Err(HostBridgeError::State(
            "host state marker exceeds its byte bound".to_string(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| HostBridgeError::State("host state marker JSON is invalid".to_string()))
}

pub(super) fn require_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| HostBridgeError::Io(format!("inspect {label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(HostBridgeError::State(format!(
            "{label} is not an admitted directory"
        )));
    }
    Ok(())
}

pub(super) fn require_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| HostBridgeError::Io(format!("inspect {label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HostBridgeError::State(format!(
            "{label} is not an admitted regular file"
        )));
    }
    Ok(())
}

fn prepare_fresh_directory(path: &Path, leaf: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(HostBridgeError::State(format!(
                    "fresh state path {leaf} is not an admitted directory"
                )));
            }
            if std::fs::read_dir(path)
                .map_err(|error| {
                    HostBridgeError::Io(format!("read fresh state path {leaf}: {error}"))
                })?
                .next()
                .is_some()
            {
                return Err(HostBridgeError::State(format!(
                    "fresh state path {leaf} contains partial state"
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(|error| {
                HostBridgeError::Io(format!("create state path {leaf}: {error}"))
            })?;
        }
        Err(error) => {
            return Err(HostBridgeError::Io(format!(
                "inspect state path {leaf}: {error}"
            )));
        }
    }
    Ok(())
}
