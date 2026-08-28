use std::fs::File;
use std::io::Read;
use std::path::Path;

use durable_file_publication::core::{
    DurabilityMode, PublicationDisposition, PublicationRequest, ReplacementMode,
};
use durable_file_publication::shell::{LinuxDirectory, StageNameSource, publish_one_file};
use rustix::fs::{Mode, OFlags, openat};

use super::model::{HostedProviderError, HostedProviderResult};

const REPORT_FILE_MODE: u32 = 0o600;

#[derive(Debug)]
struct BoundedStageNames {
    prefix: String,
    next: usize,
}

impl BoundedStageNames {
    fn new(payload: &[u8]) -> Self {
        Self {
            prefix: blake3::hash(payload).to_hex().to_string(),
            next: 0,
        }
    }
}

impl StageNameSource for BoundedStageNames {
    fn next_stage_leaf(&mut self) -> String {
        let selected = self.next;
        self.next = self.next.saturating_add(1);
        format!("stage-{}-{selected}", self.prefix)
    }
}

pub(super) fn publish_exact(
    directory_path: &Path,
    leaf: &str,
    bytes: &[u8],
    maximum_bytes: usize,
    collision_attempts: usize,
) -> HostedProviderResult<()> {
    let directory = File::open(directory_path).map_err(|error| {
        HostedProviderError::Infrastructure(format!("open report directory: {error}"))
    })?;
    let mut publication = LinuxDirectory::from_open_directory(directory).map_err(|error| {
        HostedProviderError::Infrastructure(format!("admit report directory: {error}"))
    })?;
    let request = PublicationRequest {
        destination_leaf: leaf.to_string(),
        payload_bytes: bytes.len(),
        maximum_payload_bytes: maximum_bytes,
        collision_attempt_limit: collision_attempts,
        final_mode: REPORT_FILE_MODE,
        replacement: ReplacementMode::NoReplace,
        durability: DurabilityMode::DurabilityRequired,
    };
    let mut names = BoundedStageNames::new(bytes);
    let classification =
        publish_one_file(&mut publication, &mut names, &request, bytes).map_err(|error| {
            HostedProviderError::Infrastructure(format!("classify report publication: {error:?}"))
        })?;
    match classification.disposition {
        PublicationDisposition::CommittedAndParentSynchronized => Ok(()),
        PublicationDisposition::DestinationExists { .. } => {
            let existing = read_relative_file(directory_path, leaf, maximum_bytes)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(HostedProviderError::Admission(
                    "report destination contains conflicting bytes".to_string(),
                ))
            }
        }
        PublicationDisposition::NotCommitted { .. }
        | PublicationDisposition::CommittedVisibilityOnly
        | PublicationDisposition::CommittedDurabilityUnknown => {
            Err(HostedProviderError::Infrastructure(
                "report publication did not reach the required durable state".to_string(),
            ))
        }
    }
}

fn read_relative_file(
    directory_path: &Path,
    leaf: &str,
    maximum_bytes: usize,
) -> HostedProviderResult<Vec<u8>> {
    let directory = File::open(directory_path).map_err(|error| {
        HostedProviderError::Infrastructure(format!("open report root for read: {error}"))
    })?;
    let descriptor = openat(
        &directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        HostedProviderError::Infrastructure(format!("open existing report: {error}"))
    })?;
    let mut file = File::from(descriptor);
    let metadata = file.metadata().map_err(|error| {
        HostedProviderError::Infrastructure(format!("inspect existing report: {error}"))
    })?;
    if !metadata.is_file()
        || metadata.len()
            > u64::try_from(maximum_bytes).map_err(|_| {
                HostedProviderError::Admission(
                    "existing report bound does not fit this host".to_string(),
                )
            })?
    {
        return Err(HostedProviderError::Admission(
            "existing report is not a bounded regular file".to_string(),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        HostedProviderError::Infrastructure(format!("read existing report: {error}"))
    })?;
    Ok(bytes)
}
