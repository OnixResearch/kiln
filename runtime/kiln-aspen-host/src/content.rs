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

use kiln_adapter_aspen::{AspenContentPort, CONTENT_REFERENCE_KIND, CONTENT_VERIFICATION_KIND};
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure};
use molten::chunk_store::{
    CapabilityChunkRoot, open_capability_chunk_root, put_bytes_with_root, read_manifest_with_root,
};
use molten::content_store_adapter::{
    ContentAdapterClass, ContentAdapterProfile, ContentCommandInput, ContentOperation,
    ContentResourceBounds, LiveIrohIdentity, LiveIrohPublication, assemble_verified_content,
    content_adapter_profile, content_command, execute_live_iroh_stream_get,
    execute_local_stream_get, execute_local_stream_put, manifest_descriptor,
    publish_live_iroh_chunks,
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

    /// Publishes the port's exact manifest over a live Iroh blobs provider.
    pub async fn publish_live(
        &mut self,
        reference: &ExternalRef,
        identity: LiveIrohIdentity<'_>,
    ) -> Result<(), PortFailure> {
        Self::verify_reference_shape(reference)?;
        let digest = reference.native_id.as_str();
        let manifest_ref = self.index.get(digest).ok_or_else(|| {
            infrastructure(
                "aspen_content_missing",
                "Aspen content object is not available",
            )
        })?;
        let stored = read_manifest_with_root(&self.root, manifest_ref)
            .map_err(molten_failure("aspen_content_manifest"))?;
        let manifest = manifest_descriptor(&stored);
        let publication = publish_live_iroh_chunks(
            &self.live_profile,
            &self.root,
            &manifest.manifest_ref,
            identity,
        )
        .await
        .map_err(molten_failure("aspen_content_live_publish"))?;
        self.live = Some(publication);
        Ok(())
    }

    /// Reads the currently published live manifest back through Iroh.
    pub async fn get_live_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure> {
        Self::verify_reference_shape(reference)?;
        if maximum_bytes == 0 {
            return Err(domain(
                "aspen_content_zero_bound",
                "aspen.content.maximum_bytes",
                "content read bound must be nonzero",
            ));
        }
        let publication = self.live.as_ref().ok_or_else(|| {
            infrastructure(
                "aspen_content_live_missing",
                "Aspen live content publication is not available",
            )
        })?;
        let manifest = publication.manifest();
        if manifest.total_length > maximum_bytes as u64 {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let digest = reference.native_id.as_str();
        let command =
            self.command_for(&self.live_profile, ContentOperation::Get, manifest, digest)?;
        let execution = execute_live_iroh_stream_get(
            &self.live_profile,
            publication,
            &command,
            1,
            None,
            std::time::Duration::from_secs(10),
        )
        .await
        .map_err(molten_failure("aspen_content_live_get"))?;
        let bytes = assemble_verified_content(
            manifest,
            &execution.state.artifact,
            &execution.verified_chunks,
        )
        .map_err(molten_failure("aspen_content_live_assemble"))?;
        if bytes.len() > maximum_bytes {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let observed = blake3::hash(&bytes).to_hex().to_string();
        if observed.as_str() != digest {
            return Err(infrastructure(
                "aspen_content_corrupt",
                "Aspen content bytes fail exact BLAKE3 verification",
            ));
        }
        Ok(bytes)
    }
}

impl AspenContentPort for MoltenContentPort {
    // r[impl kiln.aspen_runtime.content] r[impl kiln.aspen_runtime.identity]
    fn put_exact(
        &mut self,
        reference: &ExternalRef,
        bytes: &[u8],
    ) -> Result<ExternalRef, PortFailure> {
        Self::verify_reference_shape(reference)?;
        let digest = blake3::hash(bytes).to_hex().to_string();
        if reference.native_id.as_str() != digest.as_str() {
            return Err(domain(
                "aspen_content_identity",
                "aspen.content.reference",
                "content reference does not match the exact supplied bytes",
            ));
        }
        if let Some(manifest_ref) = self.index.get(&digest) {
            let stored = read_manifest_with_root(&self.root, manifest_ref)
                .map_err(molten_failure("aspen_content_manifest"))?;
            let existing =
                molten::chunk_store::read_object_with_root(&self.root, &stored.manifest_ref)
                    .map_err(molten_failure("aspen_content_read"))?;
            if existing.bytes != bytes {
                return Err(domain(
                    "aspen_content_collision",
                    "aspen.content.reference",
                    "existing content under the same reference has different bytes",
                ));
            }
        } else {
            let put = put_bytes_with_root(&self.root, OBJECT_KIND, bytes, self.chunk_size)
                .map_err(molten_failure("aspen_content_put"))?;
            let stored = read_manifest_with_root(&self.root, &put.manifest_ref)
                .map_err(molten_failure("aspen_content_manifest"))?;
            let manifest = manifest_descriptor(&stored);
            let command = self.command(ContentOperation::Put, &manifest, &digest)?;
            execute_local_stream_put(
                &self.profile,
                &self.root,
                &command,
                &manifest,
                OBJECT_KIND,
                bytes,
            )
            .map_err(molten_failure("aspen_content_put_verified"))?;
            self.index.insert(digest, put.manifest_ref);
            self.persist_index()?;
        }
        verification_receipt(reference, bytes.len())
    }

    // r[impl kiln.aspen_runtime.content]
    fn get_exact(
        &self,
        reference: &ExternalRef,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PortFailure> {
        Self::verify_reference_shape(reference)?;
        if maximum_bytes == 0 {
            return Err(domain(
                "aspen_content_zero_bound",
                "aspen.content.maximum_bytes",
                "content read bound must be nonzero",
            ));
        }
        let digest = reference.native_id.as_str();
        let manifest_ref = self.index.get(digest).ok_or_else(|| {
            infrastructure(
                "aspen_content_missing",
                "Aspen content object is not available",
            )
        })?;
        let stored = read_manifest_with_root(&self.root, manifest_ref)
            .map_err(molten_failure("aspen_content_manifest"))?;
        let manifest = manifest_descriptor(&stored);
        if manifest.total_length > maximum_bytes as u64 {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let command = self.command(ContentOperation::Get, &manifest, digest)?;
        let execution = execute_local_stream_get(&self.profile, &self.root, &command, 1, None)
            .map_err(molten_failure("aspen_content_get_verified"))?;
        let bytes = assemble_verified_content(
            &execution.manifest,
            &execution.state.artifact,
            &execution.verified_chunks,
        )
        .map_err(molten_failure("aspen_content_assemble"))?;
        if bytes.len() > maximum_bytes {
            return Err(domain(
                "aspen_content_bound",
                "aspen.content.maximum_bytes",
                "Aspen content object exceeds the admitted read bound",
            ));
        }
        let observed = blake3::hash(&bytes).to_hex().to_string();
        if observed.as_str() != digest {
            return Err(infrastructure(
                "aspen_content_corrupt",
                "Aspen content bytes fail exact BLAKE3 verification",
            ));
        }
        Ok(bytes)
    }
}

fn build_profile(chunk_size: u64) -> Result<ContentAdapterProfile, PortFailure> {
    build_profile_for(ContentAdapterClass::CapabilityLocal, chunk_size)
}

fn build_profile_for(
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

fn load_index(root: &Path) -> Result<BTreeMap<String, String>, PortFailure> {
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

fn verification_receipt(
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

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: impl Into<String>) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.into(),
    })
}

fn molten_failure(code: &'static str) -> impl Fn(molten::error::MoltenError) -> PortFailure {
    move |error| infrastructure(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(bytes: &[u8]) -> ExternalRef {
        let Ok(reference) = ExternalRef::new(
            "molten",
            CONTENT_REFERENCE_KIND,
            blake3::hash(bytes).to_hex().as_ref(),
        ) else {
            unreachable!("test reference must be valid");
        };
        reference
    }

    fn port() -> (tempfile::TempDir, MoltenContentPort) {
        let Ok(directory) = tempfile::tempdir() else {
            unreachable!("test directory must be available");
        };
        let Ok(port) = MoltenContentPort::open(directory.path(), 64) else {
            unreachable!("test content port must open");
        };
        (directory, port)
    }

    // r[verify kiln.aspen_runtime.content_publication] r[verify kiln.aspen_runtime.content_publication.scenario.publish]
    #[test]
    fn exact_round_trip_survives_reopen() {
        let (directory, mut port) = port();
        let bytes = b"kiln exact content";
        let reference = reference(bytes);
        let receipt = port.put_exact(&reference, bytes);
        assert!(
            matches!(receipt, Ok(value) if value.native_kind.as_str() == CONTENT_VERIFICATION_KIND)
        );
        let loaded = port.get_exact(&reference, 1024);
        assert!(matches!(loaded, Ok(value) if value == bytes));

        drop(port);
        let Ok(reopened) = MoltenContentPort::open(directory.path(), 64) else {
            unreachable!("content port must reopen");
        };
        let loaded = reopened.get_exact(&reference, 1024);
        assert!(matches!(loaded, Ok(value) if value == bytes));
    }

    #[test]
    fn identity_mismatch_is_rejected_before_writing() {
        let (_directory, mut port) = port();
        let Ok(wrong) = ExternalRef::new("molten", CONTENT_REFERENCE_KIND, &"0".repeat(64)) else {
            unreachable!("test reference must be valid");
        };
        let result = port.put_exact(&wrong, b"kiln exact content");
        assert!(matches!(
            result,
            Err(PortFailure::Domain(diagnostic)) if diagnostic.code.as_str() == "aspen_content_identity"
        ));
    }

    #[test]
    fn missing_content_is_an_infrastructure_failure() {
        let (_directory, port) = port();
        let reference = reference(b"not stored");
        let result = port.get_exact(&reference, 1024);
        assert!(matches!(
            result,
            Err(PortFailure::Infrastructure(failure)) if failure.code == "aspen_content_missing"
        ));
    }

    // r[verify kiln.aspen_runtime.content_publication.scenario.refused]
    #[test]
    fn read_bound_is_enforced_before_assembly() {
        let (_directory, mut port) = port();
        let bytes = b"kiln exact content";
        let reference = reference(bytes);
        assert!(port.put_exact(&reference, bytes).is_ok());
        let result = port.get_exact(&reference, 1);
        assert!(matches!(
            result,
            Err(PortFailure::Domain(diagnostic)) if diagnostic.code.as_str() == "aspen_content_bound"
        ));
    }

    fn test_ref(domain: &str) -> String {
        molten::preserves_rail::content_ref_from_bytes(domain.as_bytes())
    }

    struct LiveIdentity {
        _directory: tempfile::TempDir,
        namespace: molten::node_state::NodeStateNamespace,
        endpoint_id: String,
        handle_ref: String,
        backend_ref: String,
    }

    impl LiveIdentity {
        fn view(&self) -> LiveIrohIdentity<'_> {
            LiveIrohIdentity {
                namespace: &self.namespace,
                endpoint_id: &self.endpoint_id,
                handle_ref: &self.handle_ref,
                backend_ref: &self.backend_ref,
            }
        }
    }

    fn live_identity() -> LiveIdentity {
        let Ok(directory) = tempfile::tempdir() else {
            unreachable!("identity fixture directory");
        };
        let Ok(namespace) = molten::node_state::NodeStateNamespace::open(
            molten::node_state::NodeStateNamespaceKind::Identity,
            directory.path(),
        ) else {
            unreachable!("identity namespace");
        };
        let Ok(crypto_profile) = molten::fabric_crypto_identity::canonical_crypto_profile(
            &molten::fabric_crypto_identity::production_ed25519_profile(
                molten::preserves_rail::content_ref_from_bytes(
                    b"molten.node.transport.crypto-profile.v1",
                ),
                molten::preserves_rail::content_ref_from_bytes(
                    b"molten.node.transport.os-csprng.v1",
                ),
            ),
        ) else {
            unreachable!("crypto profile");
        };
        let backend_ref = test_ref("kiln-live-identity-backend");
        let Ok(adapter) = molten::fabric_crypto_identity::IrohEd25519FileAdapter::new(
            &namespace,
            crypto_profile,
            backend_ref.clone(),
        ) else {
            unreachable!("crypto adapter");
        };
        let Ok(resolved) = adapter.resolve_or_generate(
            molten::fabric_crypto_identity::KeyPurpose::TransportEndpoint,
            &test_ref("kiln-live-key-policy"),
            true,
        ) else {
            unreachable!("transport identity");
        };
        LiveIdentity {
            _directory: directory,
            namespace,
            endpoint_id: format!("iroh:{}", resolved.public_key),
            handle_ref: resolved.handle.handle.handle_ref.clone(),
            backend_ref,
        }
    }

    #[tokio::test]
    async fn live_iroh_blobs_transfer_preserves_exact_identity() {
        let Ok(chunk_directory) = tempfile::tempdir() else {
            unreachable!("chunk fixture directory");
        };
        let identity = live_identity();
        let Ok(root) = open_capability_chunk_root(chunk_directory.path()) else {
            unreachable!("chunk fixture root");
        };
        let bytes = b"kiln live iroh blobs";
        let Ok(put) = put_bytes_with_root(&root, OBJECT_KIND, bytes, 8) else {
            unreachable!("live fixture put");
        };
        let Ok(stored) = read_manifest_with_root(&root, &put.manifest_ref) else {
            unreachable!("live fixture manifest");
        };
        let manifest = manifest_descriptor(&stored);
        let Ok(profile) = build_profile_for(ContentAdapterClass::IrohBlobs, 8) else {
            unreachable!("live Iroh profile");
        };

        let publication = match publish_live_iroh_chunks(
            &profile,
            &root,
            &manifest.manifest_ref,
            identity.view(),
        )
        .await
        {
            Ok(publication) => publication,
            Err(error) => unreachable!("live Iroh publication: {error:?}"),
        };
        assert_eq!(publication.manifest().manifest_ref, manifest.manifest_ref);

        let Ok(command) = content_command(
            &profile,
            ContentCommandInput {
                operation_ref: test_ref("kiln-live-get"),
                operation: ContentOperation::Get,
                manifest: publication.manifest(),
                range: None,
                submitted_tick: 0,
                deadline_tick: 1,
                retry_count: 0,
                cancelled: false,
                policy_refs: Vec::new(),
            },
        ) else {
            unreachable!("live Iroh get command");
        };
        let Ok(execution) = molten::content_store_adapter::execute_live_iroh_stream_get(
            &profile,
            &publication,
            &command,
            1,
            None,
            std::time::Duration::from_secs(10),
        )
        .await
        else {
            unreachable!("live Iroh stream");
        };
        let Ok(assembled) = assemble_verified_content(
            publication.manifest(),
            &execution.state.artifact,
            &execution.verified_chunks,
        ) else {
            unreachable!("live Iroh assembly");
        };
        assert_eq!(assembled, bytes);
        publication.shutdown().await.ok();
    }

    #[tokio::test]
    async fn live_iroh_blobs_publishes_the_port_store() {
        let (_directory, mut port) = port();
        let bytes = b"kiln live port blobs";
        let reference = reference(bytes);
        assert!(port.put_exact(&reference, bytes).is_ok());
        let identity = live_identity();
        let published = port.publish_live(&reference, identity.view()).await;
        assert!(matches!(published, Ok(())));
        let loaded = port.get_live_exact(&reference, 1024).await;
        assert!(matches!(loaded, Ok(value) if value == bytes));
    }
}
