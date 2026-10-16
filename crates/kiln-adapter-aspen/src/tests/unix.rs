use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::time::Duration;

use super::super::*;
use super::support::{admitted, reference};

const UNIX_TEST_TIMEOUT_SECONDS: u64 = 5;

struct ServerFixture {
    profile_identity: kiln_core::types::ContentId,
    generation: u64,
    operation_id: kiln_core::types::ExternalRef,
    trigger_identity: kiln_core::types::ContentId,
    run_id: kiln_core::types::RunId,
}

fn spawn_exact_server(
    listener: UnixListener,
    fixture: ServerFixture,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
        let mut request = Vec::new();
        stream
            .read_to_end(&mut request)
            .unwrap_or_else(|_| unreachable!("read"));
        let decoded = decode_aspen_unix_request(&request, DEFAULT_MAX_CALLBACK_INPUT_BYTES)
            .unwrap_or_else(|_| unreachable!("decode server request"));
        assert_eq!(decoded.operation, AspenUnixOperation::Submit);
        assert_eq!(decoded.operation_id, fixture.operation_id);
        let ack = AspenServiceAck {
            schema: SERVICE_ACK_SCHEMA.to_string(),
            operation_id: fixture.operation_id.clone(),
            generation: fixture.generation,
            trigger_identity: fixture.trigger_identity,
            run_id: Some(fixture.run_id),
            decision: AspenServiceDecision::Accepted,
            receipt_ref: reference("molten", "service-receipt", "unix-accepted"),
        };
        let response = encode_aspen_unix_response(
            &AspenUnixResponse {
                schema: EXCHANGE_RESPONSE_SCHEMA.to_string(),
                profile_identity: fixture.profile_identity,
                operation_id: fixture.operation_id,
                outcome: ServiceTransportOutcome::Acknowledged(ack),
            },
            DEFAULT_MAX_CALLBACK_OUTPUT_BYTES,
        )
        .unwrap_or_else(|_| unreachable!("encode server response"));
        stream
            .write_all(&response)
            .unwrap_or_else(|_| unreachable!("write"));
    })
}

// r[verify kiln.aspen_runtime.ingress] r[verify kiln.aspen_runtime.composition]
#[test]
fn unix_aspen_transport_accepts_exact_response_and_denies_missing_endpoint() {
    let profile = admitted();
    let ingress = build_ingress(&profile, IngressSource::Radicle, trigger_request())
        .unwrap_or_else(|_| unreachable!("ingress"));
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary socket root"));
    let socket_path = directory.path().join("aspen.sock");
    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|_| unreachable!("bind socket"));
    let server = spawn_exact_server(
        listener,
        ServerFixture {
            profile_identity: profile.profile_identity.clone(),
            generation: profile.profile.generation,
            operation_id: ingress.operation_id.clone(),
            trigger_identity: ingress.trigger_identity.clone(),
            run_id: kiln_core::types::RunId::new("kiln-aspen-unix-run")
                .unwrap_or_else(|_| unreachable!("run id")),
        },
    );
    let mut transport = UnixAspenServiceTransport::new(
        socket_path,
        profile.profile.bounds.max_callback_input_bytes,
        Duration::from_secs(UNIX_TEST_TIMEOUT_SECONDS),
    )
    .unwrap_or_else(|_| unreachable!("transport"));
    assert!(matches!(
        transport.submit(&profile, &ingress),
        ServiceTransportOutcome::Acknowledged(_)
    ));
    server.join().unwrap_or_else(|_| unreachable!("server"));

    let mut unavailable = UnixAspenServiceTransport::new(
        directory.path().join("missing.sock"),
        profile.profile.bounds.max_callback_input_bytes,
        Duration::from_secs(UNIX_TEST_TIMEOUT_SECONDS),
    )
    .unwrap_or_else(|_| unreachable!("missing transport"));
    assert_eq!(
        unavailable.submit(&profile, &ingress),
        ServiceTransportOutcome::Unavailable,
    );
}

// r[verify kiln.aspen_host.transport.rejected]
#[test]
fn unix_server_codec_rejects_operation_shape_unknown_fields_and_bounds() {
    let profile = admitted();
    let ingress = build_ingress(&profile, IngressSource::Radicle, trigger_request())
        .unwrap_or_else(|_| unreachable!("ingress"));
    let mismatched = AspenUnixRequest {
        schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
        operation: AspenUnixOperation::Reconcile,
        profile_identity: profile.profile_identity.clone(),
        operation_id: ingress.operation_id.clone(),
        ingress: Some(ingress.clone()),
    };
    let mismatched_bytes = serde_json::to_vec(&mismatched)
        .unwrap_or_else(|_| unreachable!("mismatched request encoding"));
    assert!(
        decode_aspen_unix_request(&mismatched_bytes, DEFAULT_MAX_CALLBACK_INPUT_BYTES).is_err()
    );

    let valid = AspenUnixRequest {
        schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
        operation: AspenUnixOperation::Submit,
        profile_identity: profile.profile_identity,
        operation_id: ingress.operation_id.clone(),
        ingress: Some(ingress),
    };
    let mut unknown = serde_json::to_value(valid).unwrap_or_else(|_| unreachable!("request value"));
    let serde_json::Value::Object(fields) = &mut unknown else {
        unreachable!("request object");
    };
    fields.insert("unknown".to_string(), serde_json::Value::Bool(true));
    let unknown_bytes =
        serde_json::to_vec(&unknown).unwrap_or_else(|_| unreachable!("unknown request encoding"));
    assert!(decode_aspen_unix_request(&unknown_bytes, DEFAULT_MAX_CALLBACK_INPUT_BYTES).is_err());
    assert!(decode_aspen_unix_request(&unknown_bytes, 1).is_err());
}

fn trigger_request() -> kiln_provider_api::TriggerRequest {
    kiln_provider_api::TriggerRequest {
        repository: reference("radicle", "rid", "rad:aspen-unix"),
        actor: reference("radicle", "did", "did:key:aspen-unix"),
        before: Some(reference(
            "radicle",
            "oid",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )),
        after: reference("radicle", "oid", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        branch: None,
        patch: None,
        event: kiln_provider_api::TriggerEventKind::Push,
    }
}
