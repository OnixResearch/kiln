use std::io::{Cursor, Read};
use std::sync::Arc;
use std::time::Duration;

use kiln_core::types::{ContentId, Diagnostic};
use kiln_provider_api::{EventStoreSnapshot, InfrastructureFailure, PortFailure};
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::admission::{StoredLease, StoredSchema};
use crate::client::{CelldRuntime, DispatchOutcome, OperationObservation};
use crate::profile::CelldProfile;
use crate::protocol::{AuthEnvelope, WorkerResponse, sign_envelope};

/// The shell supplies an admitted HMAC key and the trusted HTTPS origin's CA.
/// The exact installed PEM bytes bind the selected and observed ingress identity.
/// No key bytes are written to the Worker manifest or included in diagnostics.
/// Redirects, ambient proxies, plain HTTP, and request-supplied origins are disabled.
pub struct HttpCelldRuntime {
    agent: ureq::Agent,
    secret: Zeroizing<Vec<u8>>,
    ingress_ca_blake3: String,
    timeout_millis: u64,
}

impl std::fmt::Debug for HttpCelldRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpCelldRuntime")
            .field("secret", &"[REDACTED]")
            .field("timeout_millis", &self.timeout_millis)
            .finish_non_exhaustive()
    }
}

impl HttpCelldRuntime {
    pub fn new(
        secret: Vec<u8>,
        trusted_ca_pem: &[u8],
        timeout_millis: u64,
    ) -> Result<Self, PortFailure> {
        if secret.is_empty() || timeout_millis == 0 {
            return Err(domain(
                "celld_http_profile",
                "a secret and deadline are required",
            ));
        }
        let certificates = rustls_pemfile::certs(&mut Cursor::new(trusted_ca_pem))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| domain("celld_ca", "trusted origin certificate is invalid"))?;
        if certificates.is_empty() {
            return Err(domain("celld_ca", "trusted origin certificate is missing"));
        }
        let mut roots = rustls::RootCertStore::empty();
        for certificate in certificates {
            roots.add(certificate).map_err(|_| {
                domain("celld_ca", "trusted origin certificate cannot be installed")
            })?;
        }
        let mut ingress_hasher = blake3::Hasher::new_derive_key("kiln.celld.ingress-ca.v1");
        ingress_hasher.update(trusted_ca_pem);
        let ingress_ca_blake3 = ingress_hasher.finalize().to_hex().to_string();
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| domain("celld_tls", "TLS 1.3 could not be initialized"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
        let agent = ureq::AgentBuilder::new()
            .try_proxy_from_env(false)
            .https_only(true)
            .redirects(0)
            .timeout(Duration::from_millis(timeout_millis))
            .tls_config(Arc::new(tls))
            .build();
        Ok(Self {
            agent,
            secret: Zeroizing::new(secret),
            ingress_ca_blake3,
            timeout_millis,
        })
    }

    fn send(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<(u16, Vec<u8>), PortFailure> {
        if profile.bounds.timeout_millis != self.timeout_millis
            || !profile.redirects_disabled
            || envelope.version != crate::WORKER_PROTOCOL_VERSION
        {
            return Err(domain(
                "celld_http_profile",
                "runtime differs from the admitted transport cohort",
            ));
        }
        if profile.selected_cohort.ingress_ca_blake3.as_str() != self.ingress_ca_blake3.as_str()
            || profile.observed_cohort.identity.ingress_ca_blake3.as_str()
                != self.ingress_ca_blake3.as_str()
        {
            return Err(domain(
                "celld_ingress_ca",
                "installed TLS trust root differs from the selected or observed ingress CA",
            ));
        }
        let authority = profile.origin.strip_prefix("https://").ok_or_else(|| {
            domain(
                "celld_origin",
                "the Worker requires a complete HTTPS origin",
            )
        })?;
        if authority.is_empty()
            || !authority
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'))
            || !matches!(
                envelope.path.as_str(),
                "/v1/append"
                    | crate::protocol::READ_PATH
                    | crate::protocol::RECONCILE_PATH
                    | crate::protocol::LEASE_PATH
                    | crate::protocol::MIGRATE_PATH
            )
        {
            return Err(domain(
                "celld_origin",
                "the Worker origin or route is not admitted",
            ));
        }
        let wire = serde_json::to_vec(envelope).map_err(|_| {
            domain(
                "celld_wire_encoding",
                "the authenticated envelope could not be encoded",
            )
        })?;
        if wire.len() > profile.bounds.max_request_bytes {
            return Err(domain(
                "celld_request_bound",
                "the authenticated request exceeds its bound",
            ));
        }
        let signature = sign_envelope(&self.secret, envelope).map_err(PortFailure::Domain)?;
        let mut encoded = String::with_capacity(signature.len() * 2);
        for byte in signature {
            encoded.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            encoded.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        let url = format!("{}{}", profile.origin, envelope.path);
        let response = self
            .agent
            .post(&url)
            .set("content-type", "application/json")
            .set("x-kiln-signature", &encoded)
            .send_bytes(&wire);
        let (status, response) = match response {
            Ok(response) => (response.status(), response),
            Err(ureq::Error::Status(status, response)) => (status, response),
            Err(ureq::Error::Transport(_)) => {
                return Err(PortFailure::Unknown(infrastructure(
                    "celld_http_unknown",
                    "transport lost the authenticated Worker response; reconcile before redispatch",
                )));
            }
        };
        if (200..300).contains(&status) {
            let cohort = response.header("x-kiln-worker-cohort").ok_or_else(|| {
                PortFailure::Unknown(infrastructure(
                    "celld_cohort_response",
                    "the TLS-authenticated Worker omitted its cohort identity",
                ))
            })?;
            let mut facts = cohort.split(':');
            if facts.next() != Some(profile.celld_revision.as_str())
                || facts.next() != Some("1")
                || facts.next() != Some("1")
                || facts.next() != Some(profile.selected_cohort.deployment_id.as_str())
                || facts.next().is_some()
            {
                return Err(PortFailure::Unknown(infrastructure(
                    "celld_cohort_response",
                    "the TLS-authenticated Worker differs from the selected deployment",
                )));
            }
        }
        let bound = u64::try_from(profile.bounds.max_response_bytes).map_err(|_| {
            domain(
                "celld_response_bound",
                "the response bound is not representable",
            )
        })?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(bound.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| {
                PortFailure::Unknown(infrastructure(
                    "celld_response_unknown",
                    "the authenticated Worker response was lost",
                ))
            })?;
        if bytes.len() > profile.bounds.max_response_bytes {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_response_bound",
                "the Worker exceeded its named response bound",
            )));
        }
        Ok((status, bytes))
    }
}

#[derive(Deserialize)]
struct LookupReply {
    status: String,
    revision: Option<u64>,
    payload_identity: Option<String>,
}

#[derive(Deserialize)]
struct ReadReply {
    status: String,
    records: Vec<kiln_core::operations::JournalRecord>,
    outbox: Vec<kiln_core::operations::OutboxRecord>,
}

#[derive(Deserialize)]
struct AppendReply {
    status: Option<String>,
    revision: Option<u64>,
    code: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseReply {
    status: String,
    lease: StoredLease,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaReply {
    status: String,
    version: u16,
    identity: ContentId,
    revision: u64,
}

#[derive(Deserialize)]
struct ErrorReply {
    code: String,
}

impl HttpCelldRuntime {
    fn control_error(status: u16, bytes: &[u8]) -> PortFailure {
        if status >= 500 || (300..400).contains(&status) {
            return PortFailure::Unknown(infrastructure(
                "celld_control_unknown",
                "the signed control operation was not authoritatively answered",
            ));
        }
        let code = serde_json::from_slice::<ErrorReply>(bytes)
            .map(|reply| reply.code)
            .unwrap_or_else(|_| "celld_control_rejected".to_string());
        PortFailure::Domain(Diagnostic::new(
            &code,
            "celld.control",
            "Worker rejected the signed control operation",
        ))
    }

    pub(crate) fn lease_control(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
        expected_status: &str,
    ) -> Result<StoredLease, PortFailure> {
        let (status, bytes) = self.send(profile, envelope)?;
        if status != 200 {
            return Err(Self::control_error(status, &bytes));
        }
        let reply: LeaseReply = serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Unknown(infrastructure(
                "celld_lease_decode",
                "signed Worker lease reply was malformed",
            ))
        })?;
        if reply.status != expected_status {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_lease_status",
                "signed Worker lease reply has an unexpected status",
            )));
        }
        Ok(reply.lease)
    }

    pub(crate) fn schema_control(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
        expected_status: &str,
        allow_absent: bool,
    ) -> Result<Option<StoredSchema>, PortFailure> {
        let (status, bytes) = self.send(profile, envelope)?;
        if status == 404 && allow_absent {
            let reply: ErrorReply = serde_json::from_slice(&bytes).map_err(|_| {
                PortFailure::Unknown(infrastructure(
                    "celld_schema_decode",
                    "signed Worker schema absence was malformed",
                ))
            })?;
            if reply.code == "celld_schema_absent" {
                return Ok(None);
            }
        }
        if status != 200 {
            return Err(Self::control_error(status, &bytes));
        }
        let reply: SchemaReply = serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Unknown(infrastructure(
                "celld_schema_decode",
                "signed Worker schema reply was malformed",
            ))
        })?;
        if reply.status != expected_status {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_schema_status",
                "signed Worker schema reply has an unexpected status",
            )));
        }
        Ok(Some(StoredSchema {
            version: reply.version,
            identity: reply.identity,
            revision: reply.revision,
        }))
    }
}

impl CelldRuntime for HttpCelldRuntime {
    fn dispatch(&mut self, profile: &CelldProfile, envelope: &AuthEnvelope) -> DispatchOutcome {
        let (status, bytes) = match self.send(profile, envelope) {
            Ok(value) => value,
            Err(PortFailure::Unknown(_)) => return DispatchOutcome::Disconnected,
            Err(PortFailure::Domain(_)) => return DispatchOutcome::KeyMismatch,
            Err(_) => return DispatchOutcome::Unknown,
        };
        if (300..400).contains(&status) {
            return DispatchOutcome::Redirected;
        }
        if status >= 500 {
            return DispatchOutcome::Unknown;
        }
        let Ok(reply) = serde_json::from_slice::<AppendReply>(&bytes) else {
            return DispatchOutcome::Malformed;
        };
        match (status, reply.status.as_deref(), reply.revision) {
            (200, Some("accepted"), Some(revision)) => {
                DispatchOutcome::Response(WorkerResponse::Accepted { revision })
            }
            (200, Some("repeated"), Some(revision)) => {
                DispatchOutcome::Response(WorkerResponse::Repeated { revision })
            }
            (409, _, _) => DispatchOutcome::Response(WorkerResponse::Conflict {
                reason: reply
                    .code
                    .unwrap_or_else(|| "Worker operation conflict".to_string()),
            }),
            (400 | 401 | 403 | 413, _, _) => DispatchOutcome::Response(WorkerResponse::Rejected {
                code: reply.code.unwrap_or_else(|| "celld_rejected".to_string()),
                message: "Worker rejected this authenticated operation".to_string(),
            }),
            _ => DispatchOutcome::Malformed,
        }
    }

    fn reconcile(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<OperationObservation, PortFailure> {
        let (status, bytes) = self.send(profile, envelope)?;
        if status != 200 {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_reconcile_unknown",
                "exact operation lookup was not authoritatively answered",
            )));
        }
        let reply: LookupReply = serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Unknown(infrastructure(
                "celld_reconcile_decode",
                "exact operation lookup returned malformed evidence",
            ))
        })?;
        match reply.status.as_str() {
            "absent" => Ok(OperationObservation::Absent),
            "committed" => {
                let revision = reply.revision.ok_or_else(|| {
                    PortFailure::Unknown(infrastructure(
                        "celld_reconcile_decode",
                        "committed operation omitted its revision",
                    ))
                })?;
                let payload = reply.payload_identity.ok_or_else(|| {
                    PortFailure::Unknown(infrastructure(
                        "celld_reconcile_decode",
                        "committed operation omitted its payload identity",
                    ))
                })?;
                let payload_identity = ContentId::from_blake3_hex(payload).map_err(|_| {
                    PortFailure::Unknown(infrastructure(
                        "celld_reconcile_decode",
                        "committed operation has an invalid payload identity",
                    ))
                })?;
                Ok(OperationObservation::Committed {
                    revision,
                    payload_identity,
                })
            }
            _ => Err(PortFailure::Unknown(infrastructure(
                "celld_reconcile_decode",
                "exact operation lookup returned an unknown status",
            ))),
        }
    }

    fn load(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        let (status, bytes) = self.send(profile, envelope)?;
        if status != 200 {
            return Err(PortFailure::Infrastructure(infrastructure(
                "celld_read_rejected",
                "the authenticated Worker read was not acknowledged",
            )));
        }
        let reply: ReadReply = serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Infrastructure(infrastructure(
                "celld_read_decode",
                "the authenticated Worker read returned malformed history",
            ))
        })?;
        if reply.status != "read" {
            return Err(PortFailure::Infrastructure(infrastructure(
                "celld_read_decode",
                "the authenticated Worker did not return a read result",
            )));
        }
        Ok(EventStoreSnapshot {
            records: reply.records,
            outbox: reply.outbox,
        })
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn domain(code: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, "celld.transport", message))
}
