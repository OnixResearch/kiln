use kiln_core::types::{ContentId, EffectId, ProviderProfileId};

use super::super::*;
use super::support::{TEST_MAXIMUM_BYTES, admitted, reference, status_request};

// r[verify kiln.aspen_runtime.profile] r[verify kiln.aspen_runtime.nonclaims]
#[test]
fn exact_profile_admits_and_drift_fails_without_fallback() {
    let profile = profile_fixture();
    let first = admit_profile(&profile).unwrap_or_else(|_| unreachable!("profile admits"));
    let second = admit_profile(&profile).unwrap_or_else(|_| unreachable!("profile admits"));
    assert_eq!(first.profile_identity, second.profile_identity);
    let document = profile_document_fixture();
    assert_eq!(
        document
            .clone()
            .into_runtime_profile()
            .unwrap_or_else(|_| unreachable!("profile document")),
        profile,
    );
    let mut bad_document = document;
    bad_document.port_count = bad_document.port_count.saturating_add(1);
    assert!(bad_document.into_runtime_profile().is_err());

    let mut revision_drift = profile.clone();
    revision_drift.source_revision = "mutable-head".to_string();
    assert!(admit_profile(&revision_drift).is_err());

    let mut zero_bound = profile.clone();
    zero_bound.bounds.max_effects = 0;
    assert!(admit_profile(&zero_bound).is_err());

    let mut missing_non_claim = profile.clone();
    missing_non_claim.non_claims.pop();
    assert!(admit_profile(&missing_non_claim).is_err());

    let mut missing_port = profile;
    missing_port.ports.pop();
    assert!(admit_profile(&missing_port).is_err());
}

// r[verify kiln.aspen_runtime.identity] r[verify kiln.aspen_runtime.content]
#[test]
fn equal_digest_text_keeps_roles_and_namespaces_distinct() {
    let bytes = b"same bytes";
    let verification = reference("molten", CONTENT_VERIFICATION_KIND, "receipt");
    let output = map_content_identity(ContentIdentityInput {
        role: KilnArtifactRole::Output,
        kiln_schema: "kiln.output.v1",
        aspen_kind: CONTENT_REFERENCE_KIND,
        aspen_schema: ASPEN_CONTENT_SCHEMA,
        bytes,
        producer_effect: None,
        verification_receipt: verification.clone(),
    })
    .unwrap_or_else(|_| unreachable!("mapping"));
    let log = map_content_identity(ContentIdentityInput {
        role: KilnArtifactRole::Log,
        kiln_schema: "kiln.log.v1",
        aspen_kind: CONTENT_REFERENCE_KIND,
        aspen_schema: ASPEN_CONTENT_SCHEMA,
        bytes,
        producer_effect: None,
        verification_receipt: verification,
    })
    .unwrap_or_else(|_| unreachable!("mapping"));
    assert_eq!(output.kiln.digest, output.aspen.digest);
    assert_ne!(output.kiln.namespace, output.aspen.namespace);
    assert_ne!(output.kiln.role, log.kiln.role);
    assert_ne!(output.mapping_identity, log.mapping_identity);
    assert!(admit_mapping(&output, b"tampered").is_err());
}

// r[verify kiln.aspen_runtime.callback]
#[test]
fn callback_frames_are_canonical_bounded_and_strict() {
    let request = status_request();
    let bytes = encode_callback_request(&request, TEST_MAXIMUM_BYTES)
        .unwrap_or_else(|_| unreachable!("encode"));
    assert_eq!(
        decode_callback_request(&bytes, TEST_MAXIMUM_BYTES),
        Ok(request)
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_callback_request(&trailing, TEST_MAXIMUM_BYTES).is_err());
    assert!(decode_callback_request(&bytes, 1).is_err());
    assert!(decode_callback_request(b"not-preserves", TEST_MAXIMUM_BYTES).is_err());
}

// r[verify kiln.aspen_runtime.effects]
#[test]
fn denied_and_stale_effects_never_produce_a_fabric_plan() {
    let admitted = admitted();
    let base = EffectMappingInput {
        approved: false,
        effect_id: EffectId::new("effect-1").unwrap_or_else(|_| unreachable!("effect")),
        class: ProviderEffectClass::BoundedProcess,
        provider_profile: ProviderProfileId::new("mantle-build-v1")
            .unwrap_or_else(|_| unreachable!("provider")),
        operation_id: reference("molten", "operation", "provider-1"),
        idempotency_key: reference("kiln", "idempotency", "provider-1"),
        request_identity: ContentId::from_blake3_hex(blake3::hash(b"request").to_hex().to_string())
            .unwrap_or_else(|_| unreachable!("identity")),
        generation: admitted.profile.generation,
        authority_ref: admitted.profile.authority_ref.clone(),
        resource_ref: admitted.profile.resource_ref.clone(),
    };
    assert!(map_provider_effect(&admitted, &base).is_err());
    let mut stale = base;
    stale.approved = true;
    stale.generation = stale.generation.saturating_add(1);
    assert!(map_provider_effect(&admitted, &stale).is_err());
}

// r[verify kiln.aspen_runtime.composition]
#[test]
fn runtime_composition_is_explicit_and_never_falls_back() {
    let standalone = compose_application(RuntimeSelection::Standalone, None, None)
        .unwrap_or_else(|_| unreachable!("standalone"));
    assert!(matches!(
        standalone.runtime,
        RuntimeComposition::Standalone { .. }
    ));

    let aspen = compose_application(
        RuntimeSelection::AspenV2,
        Some(profile_fixture()),
        Some(exact_availability()),
    )
    .unwrap_or_else(|_| unreachable!("aspen"));
    assert!(matches!(aspen.runtime, RuntimeComposition::Aspen { .. }));

    let mut unavailable = exact_availability();
    unavailable.endpoint_available = false;
    assert!(
        compose_application(
            RuntimeSelection::AspenV2,
            Some(profile_fixture()),
            Some(unavailable),
        )
        .is_err()
    );
}

#[derive(serde::Deserialize)]
struct ExpectedProducerFixture {
    callback_command: String,
    callback_request_schema: String,
    generation: u64,
    operation_native_id: String,
    protocol_schema: String,
    source_revision: String,
}

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.identity]
#[test]
fn independent_consumer_accepts_the_static_producer_fixture() {
    const PRODUCER: &[u8] = include_bytes!(
        "../../../../fixtures/aspen/runtime/171f25b5a9e878390af8f20b9797f6096cca5097/producer-callback-request.preserves"
    );
    const EXPECTED: &str = include_str!(
        "../../../../fixtures/aspen/runtime/171f25b5a9e878390af8f20b9797f6096cca5097/consumer-expected.json"
    );
    let expected: ExpectedProducerFixture =
        serde_json::from_str(EXPECTED).unwrap_or_else(|_| unreachable!("expected fixture"));
    let request = decode_callback_request(PRODUCER, DEFAULT_MAX_CALLBACK_INPUT_BYTES)
        .unwrap_or_else(|_| unreachable!("producer frame"));
    assert_eq!(request.schema, expected.callback_request_schema);
    assert_eq!(request.protocol_schema, expected.protocol_schema);
    assert_eq!(request.generation, expected.generation);
    assert_eq!(
        request.operation_id.native_id.as_str(),
        expected.operation_native_id
    );
    assert_eq!(ASPEN_REVISION, expected.source_revision);
    assert_eq!(
        expected.callback_command,
        match request.command {
            CallbackCommand::Status => "status",
            _ => "unexpected",
        }
    );
}

// r[verify kiln.aspen_runtime.boundary] r[verify kiln.aspen_runtime.composition]
#[test]
fn architecture_keeps_aspen_out_of_core_and_ambient_effects_out_of_callback() {
    const CORE_MANIFEST: &str = include_str!("../../../kiln-core/Cargo.toml");
    const PROVIDER_MANIFEST: &str = include_str!("../../../kiln-provider-api/Cargo.toml");
    const CODEC_MANIFEST: &str = include_str!("../../../kiln-codec/Cargo.toml");
    const ROOT_MANIFEST: &str = include_str!("../../../../Cargo.toml");
    let callback_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kiln-aspen-extension/src/lib.rs");
    let callback_core = std::fs::read_to_string(callback_path)
        .unwrap_or_else(|_| unreachable!("callback source is available"));

    for manifest in [CORE_MANIFEST, PROVIDER_MANIFEST, CODEC_MANIFEST] {
        assert!(!manifest.contains("aspen"));
        assert!(!manifest.contains("molten"));
    }
    for forbidden in [
        "std::fs",
        "std::net",
        "std::process",
        "std::env",
        "rusqlite",
        "UnixStream",
    ] {
        assert!(!callback_core.contains(forbidden));
    }
    assert!(!ROOT_MANIFEST.contains("path = \"../aspen"));
    assert!(ROOT_MANIFEST.contains("kiln-adapter-aspen = { path ="));
}
