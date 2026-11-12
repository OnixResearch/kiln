use std::collections::BTreeMap;
use std::path::Path;

use kiln_adapter_aspen::CONTENT_VERIFICATION_KIND;
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure};
use molten::content_store_adapter::{
    ContentAdapterClass, ContentAdapterProfile, ContentResourceBounds, content_adapter_profile,
};

use super::{
    INDEX_FILE, MAXIMUM_CHUNK_COUNT, MAXIMUM_CONCURRENT_OPERATIONS, MAXIMUM_CONTENT_BYTES,
    MAXIMUM_DEADLINE_TICKS, MAXIMUM_EVENTS, MAXIMUM_MEMORY_BYTES, MAXIMUM_QUEUED_BYTES,
    MAXIMUM_RANGE_BYTES, MAXIMUM_RETRIES, MAXIMUM_STATUS_ENTRIES, PROFILE_DOMAIN, PROFILE_ID,
};

pub(super) fn build_profile(chunk_size: u64) -> Result<ContentAdapterProfile, PortFailure> {
    build_profile_for(ContentAdapterClass::CapabilityLocal, chunk_size)
}

pub(super) fn build_profile_for(
    class: ContentAdapterClass,
    chunk_size: u64,
) -> Result<ContentAdapterProfile, PortFailure> {
    content_adapter_profile(
        PROFILE_ID,
        molten::preserves_rail::content_ref_from_bytes(PROFILE_DOMAIN),
        class,
        ContentResourceBounds {
            max_total_bytes: MAXIMUM_CONTENT_BYTES,
            max_chunk_count: MAXIMUM_CHUNK_COUNT,
            max_chunk_bytes: chunk_size,
            max_range_bytes: MAXIMUM_RANGE_BYTES,
            max_concurrent_operations: MAXIMUM_CONCURRENT_OPERATIONS,
            max_queued_bytes: MAXIMUM_QUEUED_BYTES,
            max_memory_bytes: MAXIMUM_MEMORY_BYTES,
            max_deadline_ticks: MAXIMUM_DEADLINE_TICKS,
            max_retries: MAXIMUM_RETRIES,
            max_events: MAXIMUM_EVENTS,
            max_status_entries: MAXIMUM_STATUS_ENTRIES,
        },
        Vec::new(),
    )
    .map_err(molten_failure("aspen_content_profile"))
}

pub(super) fn load_index(root: &Path) -> Result<BTreeMap<String, String>, PortFailure> {
    let path = root.join(INDEX_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            infrastructure(
                "aspen_content_index_decode",
                format!("content index could not be decoded: {error}"),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(infrastructure(
            "aspen_content_index_read",
            format!("content index could not be read: {error}"),
        )),
    }
}

pub(super) fn verification_receipt(
    reference: &ExternalRef,
    byte_length: usize,
) -> Result<ExternalRef, PortFailure> {
    let verification_digest =
        blake3::hash(format!("{}\0{byte_length}", reference.native_id.as_str(),).as_bytes())
            .to_hex()
            .to_string();
    ExternalRef::new("molten", CONTENT_VERIFICATION_KIND, &verification_digest)
        .map_err(PortFailure::Domain)
}

pub(super) fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

pub(super) fn infrastructure(code: &str, message: impl Into<String>) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.into(),
    })
}

pub(super) fn molten_failure(
    code: &'static str,
) -> impl Fn(molten::error::MoltenError) -> PortFailure {
    move |error| infrastructure(code, error.to_string())
}

/// Whether a byte length exceeds a `u64` bound. A length that does not fit
/// `u64` exceeds every bound.
pub(super) fn exceeds_bound(length: usize, maximum_bytes: u64) -> bool {
    match u64::try_from(length) {
        Ok(length) => length > maximum_bytes,
        Err(_) => true,
    }
}
