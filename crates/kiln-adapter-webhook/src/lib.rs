use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{NotificationIntent, Outcome};
use kiln_core::types::{CoreResult, Diagnostic, EffectId, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, NotificationObservation, NotificationOutcome, NotificationPort,
    NotificationRequest, PortFailure,
};
use serde::{Deserialize, Serialize};

pub const WEBHOOK_BODY_SCHEMA: &str = "kiln.webhook-notification.v1";
pub const HTTPS_SCHEME: &str = "https";
pub const POST_METHOD: &str = "POST";
pub const JSON_CONTENT_TYPE: &str = "application/json";
pub const BLAKE3_SIGNATURE_ALGORITHM: &str = "BLAKE3-keyed-v1";

const STATUS_SUCCESS_MIN: u16 = 200;
const STATUS_SUCCESS_MAX: u16 = 299;
const STATUS_REDIRECT_MIN: u16 = 300;
const STATUS_REDIRECT_MAX: u16 = 399;
const STATUS_CLIENT_ERROR_MIN: u16 = 400;
const STATUS_CLIENT_ERROR_MAX: u16 = 499;
const STATUS_SERVER_ERROR_MIN: u16 = 500;
const STATUS_SERVER_ERROR_MAX: u16 = 599;
const STATUS_RATE_LIMITED: u16 = 429;
const BLAKE3_KEY_BYTES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthPolicy {
    None,
    Bearer {
        secret_reference: ExternalRef,
    },
    Blake3Signature {
        secret_reference: ExternalRef,
        header_name: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkPolicy {
    pub allow_loopback: bool,
    pub allow_private: bool,
    pub allow_link_local: bool,
    pub allow_multicast: bool,
    pub allow_unspecified: bool,
}

// r[impl kiln.webhook.profile]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebhookProfile {
    pub profile_id: ExternalRef,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub method: String,
    pub content_type: String,
    pub auth: AuthPolicy,
    pub network: NetworkPolicy,
    pub allow_redirects: bool,
    pub idempotent_receiver: bool,
    pub reconciliation_supported: bool,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_resolver_candidates: usize,
    pub deadline_ticks: u64,
    pub retry_limit: usize,
}

pub fn admit_profile(profile: &WebhookProfile) -> CoreResult<()> {
    profile.profile_id.validate()?;
    if profile.scheme != HTTPS_SCHEME
        || profile.method != POST_METHOD
        || profile.content_type != JSON_CONTENT_TYPE
        || profile.port == 0
        || profile.allow_redirects
    {
        return Err(Diagnostic::new(
            "webhook_profile_protocol",
            "webhook.profile",
            "webhook profile must use HTTPS POST JSON with redirects disabled",
        ));
    }
    if profile.host.is_empty()
        || profile.host.contains('/')
        || profile.host.contains('@')
        || profile.host.chars().any(char::is_whitespace)
        || !profile.path.starts_with('/')
        || profile.path.contains("..")
    {
        return Err(Diagnostic::new(
            "webhook_endpoint",
            "webhook.endpoint",
            "webhook endpoint host or path is malformed or contains inline authority",
        ));
    }
    if profile.max_request_bytes == 0
        || profile.max_response_bytes == 0
        || profile.max_resolver_candidates == 0
        || profile.deadline_ticks == 0
    {
        return Err(Diagnostic::new(
            "webhook_profile_bound",
            "webhook.bounds",
            "webhook request, response, resolver, and deadline bounds must be positive",
        ));
    }
    match &profile.auth {
        AuthPolicy::None => {}
        AuthPolicy::Bearer { secret_reference } => secret_reference.validate()?,
        AuthPolicy::Blake3Signature {
            secret_reference,
            header_name,
        } => {
            secret_reference.validate()?;
            if header_name.is_empty() || header_name.chars().any(char::is_whitespace) {
                return Err(Diagnostic::new(
                    "webhook_signature_header",
                    "webhook.auth.header_name",
                    "webhook signature header name is invalid",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct WebhookBody {
    schema: String,
    run_id: String,
    outcome: String,
    notification_policy: ExternalRef,
    endpoint_profile: ExternalRef,
    idempotency_key: ExternalRef,
}

// r[impl kiln.webhook.request]
pub fn project_notification(
    profile: &WebhookProfile,
    intent: &NotificationIntent,
    effect_id: EffectId,
    notification_policy: ExternalRef,
    idempotency_key: ExternalRef,
) -> CoreResult<NotificationRequest> {
    admit_profile(profile)?;
    notification_policy.validate()?;
    idempotency_key.validate()?;
    let body = WebhookBody {
        schema: WEBHOOK_BODY_SCHEMA.to_string(),
        run_id: intent.run_id.as_str().to_string(),
        outcome: outcome_name(intent.outcome).to_string(),
        notification_policy: notification_policy.clone(),
        endpoint_profile: profile.profile_id.clone(),
        idempotency_key: idempotency_key.clone(),
    };
    let encoded = serde_json::to_vec(&body).map_err(|_| {
        Diagnostic::new(
            "webhook_body_encoding",
            "webhook.body",
            "webhook body encoding failed",
        )
    })?;
    if encoded.len() > profile.max_request_bytes {
        return Err(Diagnostic::new(
            "webhook_request_bound",
            "webhook.body",
            "webhook body exceeds the endpoint profile bound",
        ));
    }
    let body_identity = identity_from_bytes(&encoded).map_err(|_| {
        Diagnostic::new(
            "webhook_body_identity",
            "webhook.body",
            "webhook body BLAKE3 identity failed",
        )
    })?;
    Ok(NotificationRequest {
        effect_id,
        run_id: intent.run_id.clone(),
        outcome: intent.outcome,
        notification_policy,
        endpoint_profile: profile.profile_id.clone(),
        body_identity,
        idempotency_key,
        body: encoded,
    })
}

pub fn admit_request(profile: &WebhookProfile, request: &NotificationRequest) -> CoreResult<()> {
    admit_profile(profile)?;
    if request.endpoint_profile != profile.profile_id {
        return Err(Diagnostic::new(
            "webhook_dynamic_destination",
            "webhook.endpoint_profile",
            "webhook request cannot supply or change its destination profile",
        ));
    }
    if request.body.len() > profile.max_request_bytes {
        return Err(Diagnostic::new(
            "webhook_request_bound",
            "webhook.body",
            "webhook body exceeds the endpoint profile bound",
        ));
    }
    let body_identity = identity_from_bytes(&request.body).map_err(|_| {
        Diagnostic::new(
            "webhook_body_identity",
            "webhook.body",
            "webhook body BLAKE3 identity failed",
        )
    })?;
    if body_identity != request.body_identity {
        return Err(Diagnostic::new(
            "webhook_body_identity_mismatch",
            "webhook.body_identity",
            "webhook body does not match its BLAKE3 identity",
        ));
    }
    let decoded: WebhookBody = serde_json::from_slice(&request.body).map_err(|_| {
        Diagnostic::new(
            "webhook_body_decode",
            "webhook.body",
            "webhook body is malformed",
        )
    })?;
    if decoded.schema != WEBHOOK_BODY_SCHEMA
        || decoded.run_id != request.run_id.as_str()
        || decoded.outcome != outcome_name(request.outcome)
        || decoded.notification_policy != request.notification_policy
        || decoded.endpoint_profile != request.endpoint_profile
        || decoded.idempotency_key != request.idempotency_key
    {
        return Err(Diagnostic::new(
            "webhook_request_link",
            "webhook.body",
            "webhook body does not preserve its exact request linkage",
        ));
    }
    Ok(())
}

// r[impl kiln.webhook.network]
pub fn admit_addresses(profile: &WebhookProfile, addresses: &[IpAddr]) -> CoreResult<()> {
    if addresses.is_empty() || addresses.len() > profile.max_resolver_candidates {
        return Err(Diagnostic::new(
            "webhook_resolver_bound",
            "webhook.addresses",
            "webhook resolver results are empty or exceed their named bound",
        ));
    }
    for address in addresses {
        let denied = address.is_loopback() && !profile.network.allow_loopback
            || is_private(*address) && !profile.network.allow_private
            || is_link_local(*address) && !profile.network.allow_link_local
            || address.is_multicast() && !profile.network.allow_multicast
            || address.is_unspecified() && !profile.network.allow_unspecified;
        if denied {
            return Err(Diagnostic::new(
                "webhook_address_denied",
                "webhook.addresses",
                "webhook address class is outside the admitted network policy",
            ));
        }
    }
    Ok(())
}

fn is_private(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => value.is_private(),
        IpAddr::V6(value) => value.is_unique_local(),
    }
}

fn is_link_local(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => value.is_link_local(),
        IpAddr::V6(value) => value.is_unicast_link_local(),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransportProgress {
    BeforeConnect,
    Connected,
    RequestSent,
    ResponseStarted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportResult {
    Response {
        status: u16,
        response_bytes: usize,
        tls_verified: bool,
        external_reference: Option<ExternalRef>,
    },
    Redirect {
        location: String,
    },
    TimedOut {
        progress: TransportProgress,
    },
    Disconnected {
        progress: TransportProgress,
    },
    ProviderError {
        progress: TransportProgress,
    },
}

// r[impl kiln.webhook.outcome]
pub fn classify_outcome(
    profile: &WebhookProfile,
    request: &NotificationRequest,
    result: &TransportResult,
    remaining_retries: usize,
) -> NotificationObservation {
    let (outcome, retryable, external_reference) = match result {
        TransportResult::Response {
            response_bytes,
            tls_verified,
            ..
        } if !tls_verified || *response_bytes > profile.max_response_bytes => {
            (NotificationOutcome::ProviderError, false, None)
        }
        TransportResult::Response {
            status,
            external_reference,
            ..
        } if (STATUS_SUCCESS_MIN..=STATUS_SUCCESS_MAX).contains(status) => (
            NotificationOutcome::Accepted,
            false,
            external_reference.clone(),
        ),
        TransportResult::Response { status, .. } if *status == STATUS_RATE_LIMITED => (
            NotificationOutcome::RateLimited,
            remaining_retries > 0,
            None,
        ),
        TransportResult::Response { status, .. }
            if (STATUS_CLIENT_ERROR_MIN..=STATUS_CLIENT_ERROR_MAX).contains(status) =>
        {
            (NotificationOutcome::Rejected, false, None)
        }
        TransportResult::Response { status, .. }
            if (STATUS_SERVER_ERROR_MIN..=STATUS_SERVER_ERROR_MAX).contains(status) =>
        {
            (
                NotificationOutcome::ProviderError,
                remaining_retries > 0,
                None,
            )
        }
        TransportResult::Response { status, .. }
            if (STATUS_REDIRECT_MIN..=STATUS_REDIRECT_MAX).contains(status) =>
        {
            (NotificationOutcome::Rejected, false, None)
        }
        TransportResult::Redirect { .. } => (NotificationOutcome::Rejected, false, None),
        TransportResult::TimedOut { progress } => classify_ambiguous(
            profile,
            *progress,
            remaining_retries,
            NotificationOutcome::TimedOut,
        ),
        TransportResult::Disconnected { progress }
        | TransportResult::ProviderError { progress } => classify_ambiguous(
            profile,
            *progress,
            remaining_retries,
            NotificationOutcome::ProviderError,
        ),
        TransportResult::Response { .. } => (NotificationOutcome::ProviderError, false, None),
    };
    NotificationObservation {
        effect_id: request.effect_id.clone(),
        endpoint_profile: request.endpoint_profile.clone(),
        body_identity: request.body_identity.clone(),
        outcome,
        external_reference,
        retryable,
    }
}

fn classify_ambiguous(
    profile: &WebhookProfile,
    progress: TransportProgress,
    remaining_retries: usize,
    definite_outcome: NotificationOutcome,
) -> (NotificationOutcome, bool, Option<ExternalRef>) {
    if progress < TransportProgress::RequestSent {
        return (definite_outcome, remaining_retries > 0, None);
    }
    let safe_retry =
        profile.idempotent_receiver && profile.reconciliation_supported && remaining_retries > 0;
    (NotificationOutcome::Unknown, safe_retry, None)
}

pub enum AuthMaterial {
    None,
    Bearer(Vec<u8>),
    Blake3Signature { header_name: String, value: String },
}

impl fmt::Debug for AuthMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("AuthMaterial::None"),
            Self::Bearer(_) => formatter.write_str("AuthMaterial::Bearer([redacted])"),
            Self::Blake3Signature { header_name, .. } => formatter
                .debug_struct("AuthMaterial::Blake3Signature")
                .field("header_name", header_name)
                .field("value", &"[redacted]")
                .finish(),
        }
    }
}

impl AuthMaterial {
    fn redact(&mut self) {
        match self {
            Self::None => {}
            Self::Bearer(value) => value.fill(0),
            Self::Blake3Signature { value, .. } => value.clear(),
        }
    }
}

pub trait WebhookRuntime {
    fn resolve(
        &mut self,
        host: &str,
        port: u16,
        max_candidates: usize,
    ) -> Result<Vec<IpAddr>, InfrastructureFailure>;

    fn load_secret(&mut self, reference: &ExternalRef) -> Result<Vec<u8>, InfrastructureFailure>;

    fn send(
        &mut self,
        profile: &WebhookProfile,
        request: &NotificationRequest,
        auth: &AuthMaterial,
    ) -> TransportResult;
}

#[derive(Debug)]
pub struct WebhookAdapter<R> {
    profile: WebhookProfile,
    runtime: R,
    observations: BTreeMap<EffectId, NotificationObservation>,
}

impl<R> WebhookAdapter<R> {
    #[must_use]
    pub fn new(profile: WebhookProfile, runtime: R) -> Self {
        Self {
            profile,
            runtime,
            observations: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn runtime(&self) -> &R {
        &self.runtime
    }
}

impl<R: WebhookRuntime> NotificationPort for WebhookAdapter<R> {
    fn deliver(
        &mut self,
        request: &NotificationRequest,
    ) -> Result<NotificationObservation, PortFailure> {
        admit_request(&self.profile, request).map_err(PortFailure::Domain)?;
        let addresses = self
            .runtime
            .resolve(
                &self.profile.host,
                self.profile.port,
                self.profile.max_resolver_candidates,
            )
            .map_err(PortFailure::Infrastructure)?;
        admit_addresses(&self.profile, &addresses).map_err(PortFailure::Domain)?;
        let mut auth = load_auth(&self.profile, request, &mut self.runtime)?;
        let result = self.runtime.send(&self.profile, request, &auth);
        auth.redact();
        let observation =
            classify_outcome(&self.profile, request, &result, self.profile.retry_limit);
        self.observations
            .insert(request.effect_id.clone(), observation.clone());
        Ok(observation)
    }

    fn reconcile(&mut self, effect_id: &EffectId) -> Result<NotificationObservation, PortFailure> {
        self.observations.get(effect_id).cloned().ok_or_else(|| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "webhook_observation_missing".to_string(),
                message: "webhook delivery has no retained observation".to_string(),
            })
        })
    }
}

fn load_auth<R: WebhookRuntime>(
    profile: &WebhookProfile,
    request: &NotificationRequest,
    runtime: &mut R,
) -> Result<AuthMaterial, PortFailure> {
    match &profile.auth {
        AuthPolicy::None => Ok(AuthMaterial::None),
        AuthPolicy::Bearer { secret_reference } => runtime
            .load_secret(secret_reference)
            .map(AuthMaterial::Bearer)
            .map_err(PortFailure::Infrastructure),
        AuthPolicy::Blake3Signature {
            secret_reference,
            header_name,
        } => {
            let mut secret = runtime
                .load_secret(secret_reference)
                .map_err(PortFailure::Infrastructure)?;
            if secret.len() != BLAKE3_KEY_BYTES {
                secret.fill(0);
                return Err(PortFailure::Domain(Diagnostic::new(
                    "webhook_signature_key",
                    "webhook.auth.secret_reference",
                    "BLAKE3 signature key must contain exactly 32 bytes",
                )));
            }
            let mut key = [0_u8; BLAKE3_KEY_BYTES];
            key.copy_from_slice(&secret);
            let value = blake3::keyed_hash(&key, &request.body).to_hex().to_string();
            key.fill(0);
            secret.fill(0);
            Ok(AuthMaterial::Blake3Signature {
                header_name: header_name.clone(),
                value,
            })
        }
    }
}

const fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pending => "pending",
        Outcome::Success => "success",
        Outcome::DomainFailure => "domain_failure",
        Outcome::Denied => "denied",
        Outcome::Cancelled => "cancelled",
        Outcome::TimedOut => "timed_out",
        Outcome::ProviderError => "provider_error",
        Outcome::Unknown => "unknown",
    }
}

// r[impl kiln.webhook.conformance]
// r[impl kiln.webhook.nonclaims]
#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use kiln_core::types::RunId;

    use super::*;

    const HTTPS_PORT: u16 = 443;
    const MAX_REQUEST_BYTES: usize = 4096;
    const MAX_RESPONSE_BYTES: usize = 4096;
    const MAX_RESOLVER_CANDIDATES: usize = 8;
    const DEADLINE_TICKS: u64 = 30;
    const RETRY_LIMIT: usize = 2;
    const RESPONSE_BYTES: usize = 128;
    const STATUS_ACCEPTED: u16 = 202;
    const STATUS_REJECTED: u16 = 400;
    const PUBLIC_ADDRESS: [u8; 4] = [93, 184, 216, 34];
    const PRIVATE_ADDRESS: [u8; 4] = [10, 0, 0, 1];
    const SIGNING_KEY_BYTE: u8 = 7;
    const OVERSIZED_RESPONSE_BYTES: usize = MAX_RESPONSE_BYTES + 1;

    #[derive(Debug)]
    struct FixtureRuntime {
        addresses: Vec<IpAddr>,
        result: TransportResult,
        secret: Vec<u8>,
        resolve_calls: usize,
        secret_calls: usize,
        send_calls: usize,
        observed_auth_debug: Option<String>,
    }

    impl WebhookRuntime for FixtureRuntime {
        fn resolve(
            &mut self,
            _host: &str,
            _port: u16,
            _max_candidates: usize,
        ) -> Result<Vec<IpAddr>, InfrastructureFailure> {
            self.resolve_calls = self.resolve_calls.saturating_add(1);
            Ok(self.addresses.clone())
        }

        fn load_secret(
            &mut self,
            _reference: &ExternalRef,
        ) -> Result<Vec<u8>, InfrastructureFailure> {
            self.secret_calls = self.secret_calls.saturating_add(1);
            Ok(self.secret.clone())
        }

        fn send(
            &mut self,
            _profile: &WebhookProfile,
            _request: &NotificationRequest,
            auth: &AuthMaterial,
        ) -> TransportResult {
            self.send_calls = self.send_calls.saturating_add(1);
            self.observed_auth_debug = Some(format!("{auth:?}"));
            self.result.clone()
        }
    }

    fn profile() -> WebhookProfile {
        WebhookProfile {
            profile_id: ExternalRef::new("kiln", "webhook-profile", "incident-v1")
                .unwrap_or_else(|_| unreachable!()),
            scheme: HTTPS_SCHEME.to_string(),
            host: "notifications.example.invalid".to_string(),
            port: HTTPS_PORT,
            path: "/kiln/events".to_string(),
            method: POST_METHOD.to_string(),
            content_type: JSON_CONTENT_TYPE.to_string(),
            auth: AuthPolicy::Bearer {
                secret_reference: ExternalRef::new("secret", "reference", "webhook-token")
                    .unwrap_or_else(|_| unreachable!()),
            },
            network: NetworkPolicy {
                allow_loopback: false,
                allow_private: false,
                allow_link_local: false,
                allow_multicast: false,
                allow_unspecified: false,
            },
            allow_redirects: false,
            idempotent_receiver: false,
            reconciliation_supported: false,
            max_request_bytes: MAX_REQUEST_BYTES,
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_resolver_candidates: MAX_RESOLVER_CANDIDATES,
            deadline_ticks: DEADLINE_TICKS,
            retry_limit: RETRY_LIMIT,
        }
    }

    fn request(profile: &WebhookProfile) -> NotificationRequest {
        project_notification(
            profile,
            &NotificationIntent {
                run_id: RunId::new("run-a").unwrap_or_else(|_| unreachable!()),
                outcome: Outcome::Success,
            },
            EffectId::new("notify-a").unwrap_or_else(|_| unreachable!()),
            ExternalRef::new("kiln", "notification-policy", "terminal-v1")
                .unwrap_or_else(|_| unreachable!()),
            ExternalRef::new("kiln", "idempotency", "notify-a").unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn runtime(address: [u8; 4], result: TransportResult) -> FixtureRuntime {
        FixtureRuntime {
            addresses: vec![IpAddr::V4(Ipv4Addr::from(address))],
            result,
            secret: b"top-secret".to_vec(),
            resolve_calls: 0,
            secret_calls: 0,
            send_calls: 0,
            observed_auth_debug: None,
        }
    }

    // r[verify kiln.webhook.profile.scenario.valid]
    // r[verify kiln.webhook.request.scenario.valid]
    #[test]
    fn request_projection_is_deterministic_and_exact() {
        let profile = profile();
        let first = request(&profile);
        let second = request(&profile);
        assert_eq!(first, second);
        assert!(admit_request(&profile, &first).is_ok());
    }

    // r[verify kiln.webhook.profile.scenario.dynamic]
    // r[verify kiln.webhook.request.scenario.bound]
    #[test]
    fn dynamic_destination_and_oversized_body_stop_before_effects() {
        let profile = profile();
        let mut dynamic = request(&profile);
        dynamic.endpoint_profile = ExternalRef::new("kiln", "webhook-profile", "other-v1")
            .unwrap_or_else(|_| unreachable!());
        let runtime = runtime(
            PUBLIC_ADDRESS,
            TransportResult::Response {
                status: STATUS_ACCEPTED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: true,
                external_reference: None,
            },
        );
        let mut adapter = WebhookAdapter::new(profile.clone(), runtime);
        assert!(adapter.deliver(&dynamic).is_err());
        assert_eq!(adapter.runtime().resolve_calls, 0);

        let mut oversized = request(&profile);
        oversized.body = vec![0_u8; MAX_REQUEST_BYTES.saturating_add(1)];
        assert!(adapter.deliver(&oversized).is_err());
        assert_eq!(adapter.runtime().secret_calls, 0);
    }

    // r[verify kiln.webhook.network.scenario.address]
    #[test]
    fn private_address_stops_before_secret_access() {
        let profile = profile();
        let runtime = runtime(
            PRIVATE_ADDRESS,
            TransportResult::Response {
                status: STATUS_ACCEPTED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: true,
                external_reference: None,
            },
        );
        let mut adapter = WebhookAdapter::new(profile.clone(), runtime);
        assert!(adapter.deliver(&request(&profile)).is_err());
        assert_eq!(adapter.runtime().resolve_calls, 1);
        assert_eq!(adapter.runtime().secret_calls, 0);
        assert_eq!(adapter.runtime().send_calls, 0);
    }

    // r[verify kiln.webhook.network.scenario.redirect]
    // r[verify kiln.webhook.outcome.scenario.reject]
    #[test]
    fn redirect_and_definite_rejection_are_not_followed() {
        let profile = profile();
        let request = request(&profile);
        let redirect = classify_outcome(
            &profile,
            &request,
            &TransportResult::Redirect {
                location: "https://other.invalid".to_string(),
            },
            RETRY_LIMIT,
        );
        let rejected = classify_outcome(
            &profile,
            &request,
            &TransportResult::Response {
                status: STATUS_REJECTED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: true,
                external_reference: None,
            },
            RETRY_LIMIT,
        );
        assert_eq!(redirect.outcome, NotificationOutcome::Rejected);
        assert_eq!(rejected.outcome, NotificationOutcome::Rejected);
        assert!(!redirect.retryable);
    }

    // r[verify kiln.webhook.outcome.scenario.unknown]
    // r[verify kiln.webhook.conformance.scenario.unknown]
    #[test]
    fn timeout_after_possible_acceptance_is_unknown_without_unsafe_retry() {
        let profile = profile();
        let observation = classify_outcome(
            &profile,
            &request(&profile),
            &TransportResult::TimedOut {
                progress: TransportProgress::RequestSent,
            },
            RETRY_LIMIT,
        );
        assert_eq!(observation.outcome, NotificationOutcome::Unknown);
        assert!(!observation.retryable);
    }

    #[test]
    fn rate_limit_tls_and_response_bounds_are_explicit() {
        let profile = profile();
        let request = request(&profile);
        let rate_limited = classify_outcome(
            &profile,
            &request,
            &TransportResult::Response {
                status: STATUS_RATE_LIMITED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: true,
                external_reference: None,
            },
            RETRY_LIMIT,
        );
        let tls_failure = classify_outcome(
            &profile,
            &request,
            &TransportResult::Response {
                status: STATUS_ACCEPTED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: false,
                external_reference: None,
            },
            RETRY_LIMIT,
        );
        let oversized = classify_outcome(
            &profile,
            &request,
            &TransportResult::Response {
                status: STATUS_ACCEPTED,
                response_bytes: OVERSIZED_RESPONSE_BYTES,
                tls_verified: true,
                external_reference: None,
            },
            RETRY_LIMIT,
        );
        assert_eq!(rate_limited.outcome, NotificationOutcome::RateLimited);
        assert!(rate_limited.retryable);
        assert_eq!(tls_failure.outcome, NotificationOutcome::ProviderError);
        assert_eq!(oversized.outcome, NotificationOutcome::ProviderError);
    }

    #[test]
    fn blake3_signature_is_deterministic_and_redacted() {
        let mut profile = profile();
        let secret_reference = ExternalRef::new("secret", "reference", "signing-key")
            .unwrap_or_else(|_| unreachable!());
        profile.auth = AuthPolicy::Blake3Signature {
            secret_reference,
            header_name: "x-kiln-signature".to_string(),
        };
        let request = request(&profile);
        let result = TransportResult::Response {
            status: STATUS_ACCEPTED,
            response_bytes: RESPONSE_BYTES,
            tls_verified: true,
            external_reference: None,
        };
        let mut first_runtime = runtime(PUBLIC_ADDRESS, result.clone());
        first_runtime.secret = vec![SIGNING_KEY_BYTE; BLAKE3_KEY_BYTES];
        let mut second_runtime = runtime(PUBLIC_ADDRESS, result);
        second_runtime.secret = vec![SIGNING_KEY_BYTE; BLAKE3_KEY_BYTES];
        let first =
            load_auth(&profile, &request, &mut first_runtime).unwrap_or_else(|_| unreachable!());
        let second =
            load_auth(&profile, &request, &mut second_runtime).unwrap_or_else(|_| unreachable!());
        let first_debug = format!("{first:?}");
        let second_debug = format!("{second:?}");
        match (&first, &second) {
            (
                AuthMaterial::Blake3Signature { value: first, .. },
                AuthMaterial::Blake3Signature { value: second, .. },
            ) => assert_eq!(first, second),
            _ => unreachable!(),
        }
        assert_eq!(first_debug, second_debug);
        assert!(first_debug.contains("[redacted]"));
        assert!(!first_debug.contains(&SIGNING_KEY_BYTE.to_string()));
    }

    // r[verify kiln.webhook.nonclaims.scenario.scope]
    #[test]
    fn accepted_receiver_is_bounded_and_secret_debug_is_redacted() {
        let profile = profile();
        let runtime = runtime(
            PUBLIC_ADDRESS,
            TransportResult::Response {
                status: STATUS_ACCEPTED,
                response_bytes: RESPONSE_BYTES,
                tls_verified: true,
                external_reference: Some(
                    ExternalRef::new("receiver", "delivery", "accepted")
                        .unwrap_or_else(|_| unreachable!()),
                ),
            },
        );
        let mut adapter = WebhookAdapter::new(profile.clone(), runtime);
        let observation = adapter.deliver(&request(&profile));
        assert_eq!(
            observation.map(|value| value.outcome),
            Ok(NotificationOutcome::Accepted)
        );
        let debug = adapter
            .runtime()
            .observed_auth_debug
            .as_deref()
            .unwrap_or_else(|| unreachable!());
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("top-secret"));
    }
}
