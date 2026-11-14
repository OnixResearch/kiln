use durable_file_publication::core::{
    DurabilityMode, PublicationDisposition, PublicationRequest, ReplacementMode,
};
use durable_file_publication::shell::{PublicationIo, StageNameSource, publish_one_file};
use kiln_provider_api::PortFailure;

use super::INDEX_FILE;
use super::support::infrastructure;

const INDEX_MODE: u32 = 0o600;
const INDEX_STAGE_ATTEMPTS: usize = 1;

#[derive(Debug)]
struct IndexStageName;

impl StageNameSource for IndexStageName {
    fn next_stage_leaf(&mut self) -> String {
        format!("{INDEX_FILE}.tmp")
    }
}

// r[impl kiln.aspen_runtime.content_index_retry]
// r[impl kiln.aspen_runtime.content_index_retry.scenario.barrier]
pub(super) fn publish(io: &mut impl PublicationIo, bytes: &[u8]) -> Result<(), PortFailure> {
    let request = PublicationRequest {
        destination_leaf: INDEX_FILE.to_string(),
        payload_bytes: bytes.len(),
        maximum_payload_bytes: bytes.len(),
        collision_attempt_limit: INDEX_STAGE_ATTEMPTS,
        final_mode: INDEX_MODE,
        replacement: ReplacementMode::Replace,
        durability: DurabilityMode::DurabilityRequired,
    };
    let result = publish_one_file(io, &mut IndexStageName, &request, bytes).map_err(|error| {
        infrastructure(
            "aspen_content_index_publish",
            format!("content index publication could not be classified: {error:?}"),
        )
    })?;
    if result.disposition != PublicationDisposition::CommittedAndParentSynchronized {
        return Err(infrastructure(
            "aspen_content_index_publish",
            format!("content index publication did not complete: {result:?}"),
        ));
    }
    Ok(())
}
