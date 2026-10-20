use std::path::Path;

use kiln_observe_core::ObservationProfile;
use serde::{Deserialize, Serialize};

use crate::auth::parse_principal;
use crate::{ObserveError, ObserveResult};

pub const HTTP_OBSERVATION_PROFILE_SCHEMA_VERSION: u16 = 1;
pub const ED25519_AUTH_KIND: &str = "ed25519_request_v1";
pub const UCAN_AUTH_KIND: &str = "ucan_holder_invocation_v1";
pub const UCAN_OBSERVATION_OPERATION_PROFILE: &str = "kiln.observe.http.v1";

// r[impl kiln.observe.http.nonclaims]
pub const HTTP_OBSERVATION_NON_CLAIMS: &[&str] = &[
    "http observation does not prove transport acceptance",
    "http observation does not prove live completeness",
    "http observation does not prove control execution",
    "http observation does not prove provider correctness",
    "http observation does not prove CI success",
    "http observation does not establish release readiness",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginProfile {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsProfile {
    pub certificate_path: String,
    pub private_key_path: String,
    pub min_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthProfile {
    pub kind: String,
    pub principal: String,
    pub max_clock_skew_seconds: u64,
    pub replay_window_seconds: u64,
    pub max_replay_entries: u64,
    pub max_nonce_bytes: u64,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub ability: String,
    #[serde(default)]
    pub operation_profile: String,
    #[serde(default)]
    pub holder_keys: Vec<String>,
    #[serde(default)]
    pub proof_tokens: Vec<String>,
    #[serde(default)]
    pub revoked_proof_references: Vec<String>,
    #[serde(default)]
    pub max_proof_depth: u64,
    #[serde(default)]
    pub max_proof_count: u64,
    #[serde(default)]
    pub max_token_bytes: u64,
    #[serde(default)]
    pub max_holder_audience_bytes: u64,
    #[serde(default)]
    pub max_holder_resource_bytes: u64,
    #[serde(default)]
    pub max_holder_ability_bytes: u64,
}

impl AuthProfile {
    fn admit_kind(&self) -> ObserveResult<()> {
        match self.kind.as_str() {
            ED25519_AUTH_KIND => {
                parse_principal(&self.principal)?;
            }
            UCAN_AUTH_KIND => self.admit_ucan()?,
            _ => {
                return Err(ObserveError::new(
                    "unsupported_auth_kind",
                    "HTTP observation auth kind is unsupported",
                ));
            }
        }
        Ok(())
    }

    // r[impl kiln.observe.http.ucan_profile]
    // r[impl kiln.observe.http.ucan_profile.scenario.drift]
    fn admit_ucan(&self) -> ObserveResult<()> {
        let root = parse_principal(&self.issuer)?;
        let principal = parse_principal(&self.principal)?;
        if root != principal {
            return Err(ObserveError::new(
                "invalid_auth_principal",
                "UCAN issuer and principal public keys differ",
            ));
        }
        if self.operation_profile != UCAN_OBSERVATION_OPERATION_PROFILE {
            return Err(ObserveError::new(
                "unsupported_operation_profile",
                "UCAN observation operation profile is unsupported",
            ));
        }
        if self.resource.is_empty()
            || self.ability.is_empty()
            || self.resource.len() as u64 > self.max_holder_resource_bytes
            || self.ability.len() as u64 > self.max_holder_ability_bytes
            || self.issuer.len() as u64 > self.max_holder_audience_bytes
        {
            return Err(ObserveError::new(
                "invalid_ucan_capability",
                "UCAN capability fields are empty or exceed their bounds",
            ));
        }
        if self.max_proof_depth == 0
            || self.max_proof_depth > 16
            || self.max_proof_count == 0
            || self.max_proof_count > 16
            || self.max_token_bytes == 0
            || self.max_token_bytes > 65_536
            || self.max_holder_audience_bytes == 0
            || self.max_holder_audience_bytes > 256
            || self.max_holder_resource_bytes == 0
            || self.max_holder_resource_bytes > 256
            || self.max_holder_ability_bytes == 0
            || self.max_holder_ability_bytes > 256
        {
            return Err(ObserveError::new(
                "invalid_ucan_bounds",
                "UCAN observation auth bounds are invalid",
            ));
        }
        if self.holder_keys.is_empty()
            || self.holder_keys.len() as u64 > self.max_proof_count
            || self
                .holder_keys
                .iter()
                .any(|key| parse_principal(key).is_err())
        {
            return Err(ObserveError::new(
                "invalid_holder_keys",
                "UCAN holder keys are empty, unparseable, or exceed their bound",
            ));
        }
        if self.proof_tokens.len() as u64 > self.max_proof_count
            || self.proof_tokens.iter().any(|token| {
                token.is_empty()
                    || token.len() as u64 > self.max_token_bytes
                    || token.contains(char::is_whitespace)
            })
        {
            return Err(ObserveError::new(
                "invalid_proof_tokens",
                "UCAN proof tokens are empty, oversized, or contain whitespace",
            ));
        }
        if self
            .revoked_proof_references
            .iter()
            .any(|reference| !is_blake3_hex(reference))
        {
            return Err(ObserveError::new(
                "invalid_revocation_reference",
                "UCAN revoked proof reference is not a BLAKE3 identity",
            ));
        }
        if self.max_nonce_bytes < 32 {
            return Err(ObserveError::new(
                "invalid_nonce_bound",
                "UCAN holder nonces require at least 32 bytes",
            ));
        }
        crate::ucan_auth::verify_ucan_proof_anchors(self)?;
        Ok(())
    }
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentProfile {
    pub view_content_type: String,
    pub event_content_type: String,
    pub canonical_json: bool,
    pub max_view_bytes: u64,
    pub default_page_limit: u64,
    pub default_log_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveProfile {
    pub max_records: u64,
    pub max_dropped: u64,
    pub poll_interval_milliseconds: u64,
    pub max_stream_milliseconds: u64,
    pub max_sse_events: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpBounds {
    pub max_request_bytes: u64,
    pub max_header_bytes: u64,
    pub max_header_fields: u64,
    pub max_connections: u64,
    pub read_timeout_milliseconds: u64,
    pub write_timeout_milliseconds: u64,
    pub max_error_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpObservationProfile {
    pub schema_version: u16,
    pub profile_id: String,
    pub origin: OriginProfile,
    pub tls: TlsProfile,
    pub auth: AuthProfile,
    pub content: ContentProfile,
    pub live: LiveProfile,
    pub bounds: HttpBounds,
    pub core: ObservationProfile,
    pub non_claims: Vec<String>,
}

impl HttpObservationProfile {
    // r[impl kiln.observe.http.profile.scenario.match]
    // r[impl kiln.observe.http.profile.scenario.drift]
    pub fn admit(&self) -> ObserveResult<()> {
        if self.schema_version != HTTP_OBSERVATION_PROFILE_SCHEMA_VERSION {
            return Err(ObserveError::new(
                "unsupported_http_profile_version",
                "HTTP observation profile version is unsupported",
            ));
        }
        if self.profile_id.is_empty()
            || self.profile_id.contains('\r')
            || self.profile_id.contains('\n')
        {
            return Err(ObserveError::new(
                "invalid_http_profile_id",
                "HTTP observation profile identifier is empty or contains a line break",
            ));
        }
        self.core
            .admit()
            .map_err(|error| ObserveError::new("invalid_core_profile", error.message))?;
        if self.origin.scheme != "https" {
            return Err(ObserveError::new(
                "non_https_origin",
                "HTTP observation origin must use https",
            ));
        }
        if self.origin.host.is_empty()
            || self.origin.host != self.origin.host.to_ascii_lowercase()
            || self.origin.host.contains(char::is_whitespace)
            || self.origin.host.contains('/')
            || self.origin.host.contains(':')
        {
            return Err(ObserveError::new(
                "invalid_origin_host",
                "HTTP observation origin host is empty, uppercase, or malformed",
            ));
        }
        if self.origin.port == 0 {
            return Err(ObserveError::new(
                "invalid_origin_port",
                "HTTP observation origin port must be positive",
            ));
        }
        for path in [&self.tls.certificate_path, &self.tls.private_key_path] {
            if !Path::new(path).is_absolute() {
                return Err(ObserveError::new(
                    "relative_tls_path",
                    "HTTP observation TLS paths must be absolute",
                ));
            }
        }
        if self.tls.min_version != "tls12" && self.tls.min_version != "tls13" {
            return Err(ObserveError::new(
                "unsupported_tls_version",
                "HTTP observation TLS minimum version is unsupported",
            ));
        }
        self.auth.admit_kind()?;
        if self.auth.replay_window_seconds == 0
            || self.auth.max_replay_entries == 0
            || !(16..=128).contains(&self.auth.max_nonce_bytes)
        {
            return Err(ObserveError::new(
                "invalid_auth_bounds",
                "HTTP observation auth bounds are invalid",
            ));
        }
        if self.content.view_content_type != "application/json"
            || self.content.event_content_type != "text/event-stream"
            || !self.content.canonical_json
            || self.content.max_view_bytes == 0
            || self.content.default_page_limit == 0
            || self.content.default_log_bytes == 0
        {
            return Err(ObserveError::new(
                "invalid_content_contract",
                "HTTP observation content contract is unsupported or unbounded",
            ));
        }
        if self.content.default_page_limit > self.core.bounds.max_page_items
            || self.content.default_log_bytes > self.core.bounds.max_log_bytes
        {
            return Err(ObserveError::new(
                "content_default_exceeds_core",
                "HTTP observation defaults exceed the accepted projection bounds",
            ));
        }
        if self.live.max_records == 0
            || self.live.max_records > self.core.bounds.max_event_records
            || self.live.max_dropped == 0
            || self.live.poll_interval_milliseconds == 0
            || self.live.max_stream_milliseconds == 0
            || self.live.max_sse_events == 0
        {
            return Err(ObserveError::new(
                "invalid_live_bounds",
                "HTTP observation live bounds are invalid or exceed the projection bound",
            ));
        }
        for value in [
            self.bounds.max_request_bytes,
            self.bounds.max_header_bytes,
            self.bounds.max_header_fields,
            self.bounds.max_connections,
            self.bounds.read_timeout_milliseconds,
            self.bounds.write_timeout_milliseconds,
            self.bounds.max_error_bytes,
        ] {
            if value == 0 {
                return Err(ObserveError::new(
                    "zero_http_bound",
                    "HTTP observation bound must be positive",
                ));
            }
        }
        if self.non_claims.is_empty() {
            return Err(ObserveError::new(
                "missing_http_non_claims",
                "HTTP observation profile must declare non-claims",
            ));
        }
        for required in HTTP_OBSERVATION_NON_CLAIMS {
            if !self
                .non_claims
                .iter()
                .any(|claim| claim.as_str() == *required)
            {
                return Err(ObserveError::new(
                    "missing_required_http_non_claim",
                    "HTTP observation profile omits a required non-claim",
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn host_header(&self) -> String {
        if self.origin.port == 443 {
            self.origin.host.clone()
        } else {
            format!("{}:{}", self.origin.host, self.origin.port)
        }
    }
}
