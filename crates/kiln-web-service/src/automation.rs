use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kiln_adapter_github::events::EventPolicy;
use kiln_adapter_webhook::{
    AuthMaterial, AuthPolicy, NetworkPolicy, TransportProgress, TransportResult, WebhookProfile,
    WebhookRuntime,
};
use kiln_core::types::ExternalRef;
use kiln_provider_api::{InfrastructureFailure, NotificationRequest};
use serde::{Deserialize, Serialize};

pub const AUTOMATION_SCHEMA: &str = "kiln.web-automation.v1";
const MAX_POLICY_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationProfile {
    pub schema: String,
    pub triggers: Vec<TriggerProfile>,
    pub schedules: Vec<ScheduleProfile>,
    pub notifications: Vec<NotificationProfile>,
    pub max_records: usize,
    pub status_limit: usize,
    pub max_event_bytes: usize,
    pub max_tick_effects: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerProfile {
    pub id: String,
    pub project_id: String,
    pub reference: String,
    pub repository: EventPolicy,
    pub secret_file: PathBuf,
    pub enabled: bool,
    pub push: bool,
    pub pull_request: bool,
    pub pull_request_base: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleProfile {
    pub id: String,
    pub project_id: String,
    pub reference: String,
    pub enabled: bool,
    pub interval_ms: u64,
    pub min_interval_ms: u64,
    pub max_interval_ms: u64,
    pub next_due_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationProfile {
    pub id: String,
    pub project_id: String,
    pub enabled: bool,
    pub on_success: bool,
    pub on_failure: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub addresses: Vec<IpAddr>,
    pub bearer_secret_file: Option<PathBuf>,
    pub ca_certificate_file: Option<PathBuf>,
    pub allow_loopback: bool,
    pub allow_private: bool,
    pub timeout_ms: u64,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_resolver_candidates: usize,
}

impl AutomationProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|_| "automation policy cannot be opened")?;
        let mut bytes = Vec::new();
        file.take((MAX_POLICY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "automation policy cannot be read")?;
        if bytes.len() > MAX_POLICY_BYTES {
            return Err("automation policy exceeds byte bound".into());
        }
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "invalid automation policy")?;
        profile.admit()?;
        Ok(profile)
    }
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != AUTOMATION_SCHEMA
            || self.max_records == 0
            || self.max_records > 100_000
            || self.status_limit == 0
            || self.status_limit > 100
            || self.max_event_bytes == 0
            || self.max_event_bytes > MAX_POLICY_BYTES
            || self.max_tick_effects == 0
            || self.max_tick_effects > 16
            || self.triggers.len() + self.schedules.len() + self.notifications.len() > 128
        {
            return Err("automation policy bounds invalid".into());
        }
        let mut ids = BTreeSet::new();
        for (kind, id, project, reference) in self
            .triggers
            .iter()
            .map(|p| ("trigger", &p.id, &p.project_id, Some(&p.reference)))
            .chain(
                self.schedules
                    .iter()
                    .map(|p| ("schedule", &p.id, &p.project_id, Some(&p.reference))),
            )
            .chain(
                self.notifications
                    .iter()
                    .map(|p| ("notification", &p.id, &p.project_id, None)),
            )
        {
            if !crate::profile::identifier(id)
                || !crate::profile::identifier(project)
                || !ids.insert((kind, id))
                || reference.is_some_and(|v| !valid_reference(v))
            {
                return Err("automation registration identity invalid".into());
            }
        }
        for p in &self.triggers {
            kiln_adapter_github::events::admit_event_policy(&p.repository)
                .map_err(|_| "automation repository invalid")?;
            if p.repository.allow_fork_pull_requests
                || !crate::profile::absolute(&p.secret_file)
                || (p.pull_request
                    && p.pull_request_base
                        .as_ref()
                        .is_none_or(|v| !valid_reference(&format!("refs/heads/{v}"))))
            {
                return Err("automation trigger authority invalid".into());
            }
        }
        for p in &self.schedules {
            if p.min_interval_ms < 1000
                || p.max_interval_ms > 31_536_000_000
                || p.min_interval_ms > p.max_interval_ms
                || !(p.min_interval_ms..=p.max_interval_ms).contains(&p.interval_ms)
                || (p.enabled && p.next_due_ms.is_none())
                || p.next_due_ms.is_some_and(|due| due > 9_007_199_254_740_991)
            {
                return Err("automation schedule bounds invalid".into());
            }
        }
        for p in &self.notifications {
            p.webhook()?;
            kiln_adapter_webhook::admit_addresses(&p.webhook()?, &p.addresses)
                .map_err(|_| "notification network addresses denied")?;
            let canonical: Vec<_> = p
                .addresses
                .iter()
                .map(|address| match address {
                    IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(*address),
                    _ => *address,
                })
                .collect();
            kiln_adapter_webhook::admit_addresses(&p.webhook()?, &canonical)
                .map_err(|_| "notification mapped network addresses denied")?;
            let url = reqwest::Url::parse(&format!("https://{}:{}{}", p.host, p.port, p.path))
                .map_err(|_| "notification URL invalid")?;
            if url.host_str() != Some(p.host.as_str())
                || url.path() != p.path
                || url.query().is_some()
                || url.fragment().is_some()
                || p.host
                    .parse::<IpAddr>()
                    .is_ok_and(|ip| !p.addresses.contains(&ip))
            {
                return Err("notification destination is not an exact registered authority".into());
            }
            if p.timeout_ms == 0
                || p.timeout_ms > 30_000
                || p.max_request_bytes == 0
                || p.max_request_bytes > MAX_POLICY_BYTES
                || p.max_response_bytes == 0
                || p.max_response_bytes > MAX_POLICY_BYTES
                || p.max_resolver_candidates == 0
                || p.max_resolver_candidates > 16
                || p.host.contains([':', '?', '#', '\\'])
                || p.path.contains(['?', '#', '\\'])
                || p.path.starts_with("//")
                || !p
                    .host
                    .bytes()
                    .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'.' | b'-'))
                || p.path.bytes().any(|v| v.is_ascii_control() || v == b' ')
                || p.bearer_secret_file
                    .as_ref()
                    .is_some_and(|p| !crate::profile::absolute(p))
                || p.ca_certificate_file
                    .as_ref()
                    .is_some_and(|p| !crate::profile::absolute(p))
            {
                return Err("automation notification authority invalid".into());
            }
        }
        Ok(())
    }
}
pub(crate) fn valid_reference(value: &str) -> bool {
    value.starts_with("refs/heads/")
        && value.len() <= 512
        && !value.ends_with('/')
        && !value.contains("..")
        && !value.contains("@{")
        && !value.contains("//")
        && !value
            .bytes()
            .any(|v| v.is_ascii_control() || v.is_ascii_whitespace() || b"~^:?*[\\".contains(&v))
}
impl NotificationProfile {
    pub(crate) fn webhook(&self) -> Result<WebhookProfile, String> {
        let profile = WebhookProfile {
            profile_id: external("receiver", &self.id)?,
            scheme: "https".into(),
            host: self.host.clone(),
            port: self.port,
            path: self.path.clone(),
            method: "POST".into(),
            content_type: "application/json".into(),
            auth: if self.bearer_secret_file.is_some() {
                AuthPolicy::Bearer {
                    secret_reference: external("secret", &self.id)?,
                }
            } else {
                AuthPolicy::None
            },
            network: NetworkPolicy {
                allow_loopback: self.allow_loopback,
                allow_private: self.allow_private,
                allow_link_local: false,
                allow_multicast: false,
                allow_unspecified: false,
            },
            allow_redirects: false,
            idempotent_receiver: false,
            reconciliation_supported: false,
            max_request_bytes: self.max_request_bytes,
            max_response_bytes: self.max_response_bytes,
            max_resolver_candidates: self.max_resolver_candidates,
            deadline_ticks: self.timeout_ms,
            retry_limit: 0,
        };
        kiln_adapter_webhook::admit_profile(&profile)
            .map_err(|_| "invalid registered HTTPS receiver")?;
        Ok(profile)
    }
}
pub(crate) fn external(kind: &str, id: &str) -> Result<ExternalRef, String> {
    ExternalRef::new("kiln-web", kind, id)
        .map_err(|_| "automation external identity invalid".into())
}
pub(crate) fn now_ms() -> Result<u64, String> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "UTC clock unavailable")?
            .as_millis(),
    )
    .map_err(|_| "UTC clock overflow".into())
}
pub(crate) fn secret(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = crate::worker::files::regular(path, 65_536, true)
        .map_err(|_| "automation private credential unavailable")?;
    if bytes.is_empty() {
        return Err("automation private credential empty".into());
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomationState {
    pub policy_identity: Option<String>,
    pub settings: BTreeMap<String, RegistrationState>,
    pub sources: BTreeMap<String, SourceRecord>,
    pub deliveries: BTreeMap<String, DeliveryRecord>,
    pub requests: BTreeMap<String, String>,
    /// Next registration in the immutable admitted policy's round-robin order.
    pub next_registration: usize,
    /// Last recorded head per (repository key, reference): the exact base
    /// of the next push. Absent means a new reference (`no_base`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub heads: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationState {
    pub enabled: bool,
    pub push: bool,
    pub pull_request: bool,
    pub interval_ms: Option<u64>,
    pub next_due_ms: Option<u64>,
    pub on_success: bool,
    pub on_failure: bool,
    pub notification_after_ordinal: u64,
    pub last_status: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub id: String,
    pub kind: String,
    pub registration_id: String,
    pub received_at_ms: u64,
    pub status: String,
    pub run_id: Option<String>,
    pub error_code: Option<String>,
    pub project_id: String,
    pub reference: String,
    pub source_revision: Option<String>,
    pub candidate_identity: Option<String>,
    /// Affected decision of the repository event this source belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected: Option<kiln_core::monorepo::AffectedSet>,
    /// Changed-file summary behind `affected`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<kiln_core::monorepo::ChangedSummary>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRecord {
    pub id: String,
    pub registration_id: String,
    pub run_id: String,
    pub reserved_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub status: String,
    pub http_status: Option<u16>,
    pub error_code: Option<String>,
}

/// Real HTTPS shell: resolve once, admit every address, pin the admitted socket
/// set into the TLS client, validate the hostname/CA and never follow redirects.
/// A send error after connection may have accepted bytes and is never retried.
#[derive(Debug)]
pub(crate) struct HttpsRuntime {
    pub registration: NotificationProfile,
    pub addresses: Vec<SocketAddr>,
    pub http_status: Option<u16>,
}
impl HttpsRuntime {
    fn failure() -> InfrastructureFailure {
        InfrastructureFailure {
            code: "notification_preflight".into(),
            message: "registered HTTPS receiver preflight failed".into(),
        }
    }
}
impl WebhookRuntime for HttpsRuntime {
    fn resolve(
        &mut self,
        host: &str,
        port: u16,
        maximum: usize,
    ) -> Result<Vec<IpAddr>, InfrastructureFailure> {
        // Explicit reviewed IPs eliminate DNS rebinding and unbounded resolver
        // waits; TLS still authenticates the registered DNS hostname.
        if host != self.registration.host
            || port != self.registration.port
            || self.registration.addresses.is_empty()
            || self.registration.addresses.len() > maximum
        {
            return Err(Self::failure());
        }
        self.addresses = self
            .registration
            .addresses
            .iter()
            .map(|ip| SocketAddr::new(*ip, port))
            .collect();
        Ok(self.registration.addresses.clone())
    }
    fn load_secret(&mut self, reference: &ExternalRef) -> Result<Vec<u8>, InfrastructureFailure> {
        if external("secret", &self.registration.id).ok().as_ref() != Some(reference) {
            return Err(Self::failure());
        }
        secret(
            self.registration
                .bearer_secret_file
                .as_deref()
                .ok_or_else(Self::failure)?,
        )
        .map_err(|_| Self::failure())
    }
    fn send(
        &mut self,
        profile: &WebhookProfile,
        request: &NotificationRequest,
        auth: &AuthMaterial,
    ) -> TransportResult {
        let before = || TransportResult::ProviderError {
            progress: TransportProgress::BeforeConnect,
        };
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut builder = reqwest::blocking::Client::builder()
            .https_only(true)
            .no_proxy()
            .retry(reqwest::retry::never())
            .http1_only()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(self.registration.timeout_ms))
            .connect_timeout(Duration::from_millis(self.registration.timeout_ms))
            .resolve_to_addrs(&profile.host, &self.addresses);
        if let Some(path) = &self.registration.ca_certificate_file {
            let Ok(bytes) = crate::worker::files::regular(path, MAX_POLICY_BYTES, false) else {
                return before();
            };
            let Ok(certificate) = reqwest::Certificate::from_pem(&bytes) else {
                return before();
            };
            builder = builder.add_root_certificate(certificate);
        }
        let Ok(client) = builder.build() else {
            return before();
        };
        let url = format!("https://{}:{}{}", profile.host, profile.port, profile.path);
        let mut send = client
            .post(url)
            .header("content-type", "application/json")
            .header(
                "idempotency-key",
                request.idempotency_key.native_id.as_str(),
            )
            .body(request.body.clone());
        send = match auth {
            AuthMaterial::None => send,
            AuthMaterial::Bearer(value) => {
                let Ok(value) = reqwest::header::HeaderValue::from_bytes(value) else {
                    return before();
                };
                let Ok(value) = reqwest::header::HeaderValue::from_bytes(
                    &[b"Bearer ".as_slice(), value.as_bytes()].concat(),
                ) else {
                    return before();
                };
                send.header(reqwest::header::AUTHORIZATION, value)
            }
            AuthMaterial::Blake3Signature { header_name, value } => {
                send.header(header_name.as_str(), value.as_str())
            }
        };
        let mut response = match send.send() {
            Ok(response) => response,
            Err(error) => {
                let progress = if error.is_connect() || error.is_builder() {
                    TransportProgress::BeforeConnect
                } else {
                    TransportProgress::RequestSent
                };
                return if error.is_timeout() {
                    TransportResult::TimedOut { progress }
                } else {
                    TransportResult::Disconnected { progress }
                };
            }
        };
        let status = response.status().as_u16();
        self.http_status = Some(status);
        let mut bytes = 0;
        let mut buffer = [0_u8; 4096];
        loop {
            let remaining = profile
                .max_response_bytes
                .saturating_add(1)
                .saturating_sub(bytes);
            if remaining == 0 {
                break;
            }
            let amount = remaining.min(buffer.len());
            let Some(chunk) = buffer.get_mut(..amount) else {
                return TransportResult::Disconnected {
                    progress: TransportProgress::ResponseStarted,
                };
            };
            match response.read(chunk) {
                Ok(0) => break,
                Ok(count) => bytes += count,
                Err(_) => {
                    return TransportResult::Disconnected {
                        progress: TransportProgress::ResponseStarted,
                    };
                }
            }
        }
        if bytes > profile.max_response_bytes {
            return TransportResult::Disconnected {
                progress: TransportProgress::ResponseStarted,
            };
        }
        TransportResult::Response {
            status,
            response_bytes: bytes,
            tls_verified: true,
            external_reference: None,
        }
    }
}
