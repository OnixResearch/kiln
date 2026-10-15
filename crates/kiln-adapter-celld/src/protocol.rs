use kiln_codec::identity_from_bytes;
use kiln_core::domain::Event;
use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{EventAppendRequest, InfrastructureFailure, PortFailure};

use crate::profile::CelldProfile;

pub const ENVELOPE_HEADER: &str = "x-kiln-envelope-v1";
pub const CELLD_PARTITION_NAMESPACE: &str = "celld";
pub const CELLD_PARTITION_KIND: &str = "object-blake3";
pub const CELLD_OPERATION_KIND: &str = "operation";
pub const CELLD_PAYLOAD_KIND: &str = "payload-blake3";
pub const MAX_ENVELOPE_BODY_BYTES: usize = 4 * 1024 * 1024;
const SIGNATURE_DOMAIN: &[u8] = b"kiln-celld-envelope-v1\0";
pub const READ_PATH: &str = "/v1/read";
pub const RECONCILE_PATH: &str = "/v1/reconcile";
pub const LEASE_PATH: &str = "/v1/lease";
pub const MIGRATE_PATH: &str = "/v1/migrate";

pub fn identity_from_event(event: &Event) -> CoreResult<ContentId> {
    let bytes = serde_json::to_vec(event).map_err(|_| {
        Diagnostic::new(
            "celld_event_encoding",
            "celld.event",
            "event canonical JSON encoding failed",
        )
    })?;
    identity_from_bytes(&bytes).map_err(|_| {
        Diagnostic::new(
            "celld_event_identity",
            "celld.event",
            "event BLAKE3 identity failed",
        )
    })
}

pub fn payload_identity_of(
    profile: &CelldProfile,
    request: &EventAppendRequest,
) -> CoreResult<ContentId> {
    encode_append_payload(profile, request).map(|(_, identity)| identity)
}

pub(crate) fn encode_append_payload(
    profile: &CelldProfile,
    request: &EventAppendRequest,
) -> CoreResult<(Vec<u8>, ContentId)> {
    let bytes = serde_json::to_vec(request).map_err(|_| {
        Diagnostic::new(
            "celld_payload_encoding",
            "celld.payload",
            "operation payload canonical JSON encoding failed",
        )
    })?;
    if bytes.len() > profile.bounds.max_request_bytes {
        return Err(Diagnostic::new(
            "celld_payload_bound",
            "celld.payload",
            "operation payload bytes exceed their named bound",
        ));
    }
    let identity = identity_from_bytes(&bytes).map_err(|_| {
        Diagnostic::new(
            "celld_payload_identity",
            "celld.payload",
            "operation payload BLAKE3 identity failed",
        )
    })?;
    Ok((bytes, identity))
}

// r[impl kiln.celld.transport] r[impl kiln.celld.authority]
/// Bounded authenticated request envelope. The path binds the request to the
/// exact admitted Worker route; redirects and request-supplied destinations
/// stay disabled.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct AuthEnvelope {
    pub version: u16,
    pub path: String,
    pub key_reference: ExternalRef,
    pub timestamp: u64,
    pub nonce: String,
    pub operation_id: ExternalRef,
    pub payload_identity: ContentId,
    pub body: Vec<u8>,
}

pub fn build_envelope(
    path: &str,
    key_reference: &ExternalRef,
    timestamp: u64,
    nonce: &str,
    operation_id: &ExternalRef,
    payload_identity: &ContentId,
    body: Vec<u8>,
) -> CoreResult<AuthEnvelope> {
    if path.is_empty() {
        return Err(Diagnostic::new(
            "celld_route",
            "celld.transport.path",
            "celld transport path must not be empty",
        ));
    }
    key_reference.validate()?;
    operation_id.validate()?;
    if nonce.is_empty() {
        return Err(Diagnostic::new(
            "celld_nonce",
            "celld.transport.nonce",
            "celld transport nonce must not be empty",
        ));
    }
    if body.len() > MAX_ENVELOPE_BODY_BYTES {
        return Err(Diagnostic::new(
            "celld_envelope_bound",
            "celld.transport.body",
            "celld envelope body exceeds its named bound",
        ));
    }
    Ok(AuthEnvelope {
        version: crate::WORKER_PROTOCOL_VERSION,
        path: path.to_string(),
        key_reference: key_reference.clone(),
        timestamp,
        nonce: nonce.to_string(),
        operation_id: operation_id.clone(),
        payload_identity: payload_identity.clone(),
        body,
    })
}

fn authenticate_field(mac: &mut hmac::Hmac<sha2::Sha256>, field: &[u8]) {
    use hmac::Mac;
    mac.update(&(field.len() as u64).to_le_bytes());
    mac.update(field);
}

pub fn sign_envelope(
    secret: &[u8],
    envelope: &AuthEnvelope,
) -> Result<[u8; crate::HMAC_DIGEST_BYTES], Diagnostic> {
    use hmac::Mac;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret).map_err(|_| {
        Diagnostic::new(
            "celld_key_bytes",
            "celld.transport.key",
            "celld HMAC secret is not a valid key",
        )
    })?;
    mac.update(SIGNATURE_DOMAIN);
    authenticate_field(&mut mac, envelope.path.as_bytes());
    authenticate_field(&mut mac, &envelope.version.to_le_bytes());
    authenticate_field(
        &mut mac,
        envelope
            .key_reference
            .provider_namespace
            .as_str()
            .as_bytes(),
    );
    authenticate_field(
        &mut mac,
        envelope.key_reference.native_kind.as_str().as_bytes(),
    );
    authenticate_field(
        &mut mac,
        envelope.key_reference.native_id.as_str().as_bytes(),
    );
    authenticate_field(&mut mac, &envelope.timestamp.to_le_bytes());
    authenticate_field(&mut mac, envelope.nonce.as_bytes());
    authenticate_field(
        &mut mac,
        envelope.operation_id.provider_namespace.as_str().as_bytes(),
    );
    authenticate_field(
        &mut mac,
        envelope.operation_id.native_kind.as_str().as_bytes(),
    );
    authenticate_field(
        &mut mac,
        envelope.operation_id.native_id.as_str().as_bytes(),
    );
    authenticate_field(&mut mac, envelope.payload_identity.as_str().as_bytes());
    authenticate_field(&mut mac, &envelope.body);
    let digest = mac.finalize().into_bytes();
    let mut output = [0u8; crate::HMAC_DIGEST_BYTES];
    output.copy_from_slice(&digest);
    Ok(output)
}

pub fn verify_envelope(
    secret: &[u8],
    envelope: &AuthEnvelope,
    signature: &[u8; crate::HMAC_DIGEST_BYTES],
) -> CoreResult<()> {
    let expected = sign_envelope(secret, envelope)?;
    if *signature != expected {
        return Err(Diagnostic::new(
            "celld_signature",
            "celld.transport.signature",
            "celld envelope signature does not match the admitted key, timestamp, nonce, and body",
        ));
    }
    Ok(())
}

// r[impl kiln.celld.conformance] r[impl kiln.celld.nonclaims]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerResponse {
    Accepted { revision: u64 },
    Repeated { revision: u64 },
    Rejected { code: String, message: String },
    Conflict { reason: String },
    Damage { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationOutcome {
    pub operation_id: ExternalRef,
    pub payload_identity: ContentId,
    pub revision: u64,
}

pub fn classify_worker_response(
    operation_id: &ExternalRef,
    payload_identity: &ContentId,
    response: WorkerResponse,
) -> Result<OperationOutcome, PortFailure> {
    let revision = match response {
        WorkerResponse::Accepted { revision } | WorkerResponse::Repeated { revision } => revision,
        WorkerResponse::Rejected { code, message } => {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_rejected",
                operation_id.native_kind.as_str(),
                &format!("{code}: {message}"),
            )));
        }
        WorkerResponse::Conflict { reason } => {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_conflict",
                operation_id.native_kind.as_str(),
                &reason,
            )));
        }
        WorkerResponse::Damage { message } => {
            return Err(PortFailure::Infrastructure(InfrastructureFailure {
                code: "celld_store_damage".to_string(),
                message,
            }));
        }
    };
    Ok(OperationOutcome {
        operation_id: operation_id.clone(),
        payload_identity: payload_identity.clone(),
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_reference() -> ExternalRef {
        ExternalRef::new("celld", "hmac-key", "kiln-primary")
            .unwrap_or_else(|_| unreachable!("key builds"))
    }

    fn operation_id() -> ExternalRef {
        ExternalRef::new("kiln", "operation", "op-1").unwrap_or_else(|_| unreachable!("op builds"))
    }

    fn secret() -> [u8; 32] {
        [0x5c; 32]
    }

    #[test]
    fn signs_and_verifies_an_envelope() {
        let identity =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!("identity"));
        let envelope = build_envelope(
            "/v1/append",
            &key_reference(),
            100,
            "nonce-1",
            &operation_id(),
            &identity,
            vec![1, 2, 3],
        )
        .unwrap_or_else(|_| unreachable!("envelope builds"));
        let signature =
            sign_envelope(&secret(), &envelope).unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(verify_envelope(&secret(), &envelope, &signature), Ok(()));
    }

    #[test]
    fn length_framing_rejects_equal_legacy_nonce_and_operation_bytes() {
        let identity =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!("identity"));
        let original_operation = ExternalRef::new("bc", "operation", "op-1")
            .unwrap_or_else(|_| unreachable!("operation"));
        let first = build_envelope(
            "/v1/append",
            &key_reference(),
            100,
            "a",
            &original_operation,
            &identity,
            vec![1, 2, 3],
        )
        .unwrap_or_else(|_| unreachable!("envelope"));
        let mut changed = first.clone();
        changed.nonce = "ab".to_string();
        changed.operation_id = ExternalRef::new("c", "operation", "op-1")
            .unwrap_or_else(|_| unreachable!("operation"));
        assert_eq!(
            format!(
                "{}{}",
                first.nonce,
                first.operation_id.provider_namespace.as_str()
            ),
            format!(
                "{}{}",
                changed.nonce,
                changed.operation_id.provider_namespace.as_str()
            ),
        );
        let signature = sign_envelope(&secret(), &first).unwrap_or_else(|_| unreachable!("signs"));
        assert_ne!(
            signature,
            sign_envelope(&secret(), &changed).unwrap_or_else(|_| unreachable!("signs"))
        );
        let error = verify_envelope(&secret(), &changed, &signature)
            .err()
            .unwrap_or_else(|| unreachable!("boundary collision must not authenticate"));
        assert_eq!(error.code, "celld_signature");
    }

    #[test]
    fn rejects_a_wrong_signature() {
        let identity =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!("identity"));
        let envelope = build_envelope(
            "/v1/append",
            &key_reference(),
            100,
            "nonce-1",
            &operation_id(),
            &identity,
            vec![1, 2, 3],
        )
        .unwrap_or_else(|_| unreachable!("envelope builds"));
        let mut signature =
            sign_envelope(&secret(), &envelope).unwrap_or_else(|_| unreachable!("signs"));
        signature[0] ^= 0xff;
        let error = verify_envelope(&secret(), &envelope, &signature)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_signature");
    }

    #[test]
    fn rejects_an_empty_nonce() {
        let identity =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!("identity"));
        let error = build_envelope(
            "/v1/append",
            &key_reference(),
            100,
            "",
            &operation_id(),
            &identity,
            Vec::new(),
        )
        .err()
        .unwrap_or_else(|| unreachable!("empty nonce fails"));
        assert_eq!(error.code, "celld_nonce");
    }

    #[test]
    fn classifies_accepted_and_rejected_responses() {
        let payload =
            ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!("identity"));
        let outcome = classify_worker_response(
            &operation_id(),
            &payload,
            WorkerResponse::Accepted { revision: 7 },
        )
        .unwrap_or_else(|_| unreachable!("accepted"));
        assert_eq!(outcome.revision, 7);
        let error = classify_worker_response(
            &operation_id(),
            &payload,
            WorkerResponse::Rejected {
                code: "wrong".to_string(),
                message: "nope".to_string(),
            },
        )
        .err()
        .unwrap_or_else(|| unreachable!("rejected"));
        assert!(matches!(error, PortFailure::Domain(_)));
        let damage = classify_worker_response(
            &operation_id(),
            &payload,
            WorkerResponse::Damage {
                message: "damaged".to_string(),
            },
        )
        .err()
        .unwrap_or_else(|| unreachable!("damaged"));
        assert!(matches!(damage, PortFailure::Infrastructure(_)));
    }
}
