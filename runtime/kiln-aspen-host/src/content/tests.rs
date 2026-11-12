use kiln_adapter_aspen::{AspenContentPort, CONTENT_REFERENCE_KIND, CONTENT_VERIFICATION_KIND};
use kiln_core::types::ExternalRef;
use kiln_provider_api::PortFailure;
use molten::chunk_store::{
    open_capability_chunk_root, put_bytes_with_root, read_manifest_with_root,
};
use molten::content_store_adapter::{
    ContentAdapterClass, ContentCommandInput, ContentOperation, LiveIrohIdentity,
    assemble_verified_content, content_command, manifest_descriptor, publish_live_iroh_chunks,
};

use super::support::build_profile_for;
use super::{MoltenContentPort, OBJECT_KIND};

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
            molten::preserves_rail::content_ref_from_bytes(b"molten.node.transport.os-csprng.v1"),
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

    let publication =
        match publish_live_iroh_chunks(&profile, &root, &manifest.manifest_ref, identity.view())
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
