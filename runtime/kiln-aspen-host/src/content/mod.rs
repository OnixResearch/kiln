//! Bounded Molten-backed implementation of Kiln's Aspen content port.
//!
//! This shell stores exact Kiln content in a capability-rooted Molten chunk
//! store, exercises Molten's content-adapter preflight and verification path on
//! put/get, and persists only the BLAKE3-to-manifest mapping. It does not grant
//! authority, confidentiality, retention, or execution meaning; those remain
//! Kiln and Aspen policy decisions.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use kiln_adapter_aspen::CONTENT_REFERENCE_KIND;
use kiln_core::types::ExternalRef;
use kiln_provider_api::PortFailure;
use molten::chunk_store::{CapabilityChunkRoot, open_capability_chunk_root};
use molten::content_store_adapter::{
    ContentAdapterClass, ContentAdapterProfile, ContentCommandInput, ContentOperation,
    LiveIrohPublication, content_command,
};

mod live;
mod port;
mod support;

use support::{
    build_profile, build_profile_for, domain, infrastructure, load_index, molten_failure,
};

const INDEX_FILE: &str = "kiln-content-index.v1.json";
const OBJECT_KIND: &str = "kiln-content";
const PROFILE_ID: &str = "kiln-aspen-host-content-v1";
const PROFILE_DOMAIN: &[u8] = b"kiln.aspen-host.content.v1";
const OPERATION_DOMAIN: &[u8] = b"kiln.aspen-host.content.operation.v1";
const MAXIMUM_CONTENT_BYTES: u64 = 1_073_741_824;
const MAXIMUM_CHUNK_COUNT: usize = 1_000_000;
const MAXIMUM_RANGE_BYTES: u64 = 1_073_741_824;
const MAXIMUM_CONCURRENT_OPERATIONS: usize = 1;
const MAXIMUM_QUEUED_BYTES: u64 = 1_073_741_824;
const MAXIMUM_MEMORY_BYTES: u64 = 1_073_741_824;
const MAXIMUM_DEADLINE_TICKS: u64 = 1;
const MAXIMUM_RETRIES: u32 = 0;
const MAXIMUM_EVENTS: usize = 1_000_000;
const MAXIMUM_STATUS_ENTRIES: usize = 16;

/// Exact-content port backed by one bounded Molten chunk store.
pub struct MoltenContentPort {
    root_path: PathBuf,
    root: CapabilityChunkRoot,
    chunk_size: u64,
    index: BTreeMap<String, String>,
    profile: ContentAdapterProfile,
    live_profile: ContentAdapterProfile,
    live: Option<LiveIrohPublication>,
}

impl std::fmt::Debug for MoltenContentPort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MoltenContentPort")
            .field("root_path", &self.root_path)
            .field("chunk_size", &self.chunk_size)
            .field("index_entries", &self.index.len())
            .field("live_publication", &self.live.is_some())
            .finish_non_exhaustive()
    }
}

impl MoltenContentPort {
    /// Opens or creates the content store at `root`.
    pub fn open(root: impl AsRef<Path>, chunk_size: u64) -> Result<Self, PortFailure> {
        if chunk_size == 0 {
            return Err(domain(
                "aspen_content_zero_chunk",
                "aspen.content.chunk_size",
                "content chunk size must be nonzero",
            ));
        }
        let root_path = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root_path).map_err(|error| {
            infrastructure(
                "aspen_content_root",
                format!("content root could not be created: {error}"),
            )
        })?;
        let root =
            open_capability_chunk_root(&root_path).map_err(molten_failure("aspen_content_root"))?;
        let index = load_index(&root_path)?;
        let profile = build_profile(chunk_size)?;
        let live_profile = build_profile_for(ContentAdapterClass::IrohBlobs, chunk_size)?;
        Ok(Self {
            root_path,
            root,
            chunk_size,
            index,
            profile,
            live_profile,
            live: None,
        })
    }

    fn persist_index(&self) -> Result<(), PortFailure> {
        let encoded = serde_json::to_vec_pretty(&self.index).map_err(|error| {
            infrastructure(
                "aspen_content_index_encode",
                format!("content index could not be encoded: {error}"),
            )
        })?;
        let temporary = self.root_path.join(format!("{INDEX_FILE}.tmp"));
        let final_path = self.root_path.join(INDEX_FILE);
        let mut file = std::fs::File::create(&temporary).map_err(|error| {
            infrastructure(
                "aspen_content_index_write",
                format!("content index could not be created: {error}"),
            )
        })?;
        file.write_all(&encoded).map_err(|error| {
            infrastructure(
                "aspen_content_index_write",
                format!("content index could not be written: {error}"),
            )
        })?;
        file.sync_all().map_err(|error| {
            infrastructure(
                "aspen_content_index_sync",
                format!("content index could not be synchronized: {error}"),
            )
        })?;
        std::fs::rename(&temporary, &final_path).map_err(|error| {
            infrastructure(
                "aspen_content_index_publish",
                format!("content index could not be published: {error}"),
            )
        })
    }

    fn verify_reference_shape(reference: &ExternalRef) -> Result<(), PortFailure> {
        reference.validate().map_err(PortFailure::Domain)?;
        if reference.provider_namespace.as_str() != "molten"
            || reference.native_kind.as_str() != CONTENT_REFERENCE_KIND
        {
            return Err(domain(
                "aspen_content_identity",
                "aspen.content.reference",
                "content reference must be a Molten BLAKE3 reference",
            ));
        }
        Ok(())
    }

    fn command_for(
        &self,
        profile: &ContentAdapterProfile,
        operation: ContentOperation,
        manifest: &molten::content_store_adapter::ContentManifestDescriptor,
        digest: &str,
    ) -> Result<molten::content_store_adapter::ContentCommand, PortFailure> {
        let operation_ref = molten::preserves_rail::content_ref_from_bytes(
            format!("{}\0{digest}", String::from_utf8_lossy(OPERATION_DOMAIN)).as_bytes(),
        );
        content_command(
            profile,
            ContentCommandInput {
                operation_ref,
                operation,
                manifest,
                range: None,
                submitted_tick: 0,
                deadline_tick: MAXIMUM_DEADLINE_TICKS,
                retry_count: MAXIMUM_RETRIES,
                cancelled: false,
                policy_refs: Vec::new(),
            },
        )
        .map_err(molten_failure("aspen_content_command"))
    }

    fn command(
        &self,
        operation: ContentOperation,
        manifest: &molten::content_store_adapter::ContentManifestDescriptor,
        digest: &str,
    ) -> Result<molten::content_store_adapter::ContentCommand, PortFailure> {
        self.command_for(&self.profile, operation, manifest, digest)
    }
}

#[cfg(test)]
mod tests;
