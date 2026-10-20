#![forbid(unsafe_code)]

// r[impl kiln.observe.http.profile]
mod auth;
mod conformance;
mod live;
mod profile;
mod request;
mod response;
mod server;
mod ucan_auth;

#[cfg(test)]
mod tests;

use std::fmt::{Display, Formatter};

pub use auth::{
    AUTH_SCHEMA, AuthPayload, AuthRequest, ReplayState, body_digest, parse_principal,
    verify_auth_header,
};
pub use conformance::{
    HttpConformanceManifest, REQUIRED_NEGATIVE_CASES, REQUIRED_POSITIVE_CASES,
    admit_http_conformance,
};
pub use live::{LiveEventBuffer, LiveFrame, LiveSnapshot, sse_frame};
pub use profile::{
    AuthProfile, ContentProfile, ED25519_AUTH_KIND, HTTP_OBSERVATION_NON_CLAIMS, HttpBounds,
    HttpObservationProfile, LiveProfile, OriginProfile, TlsProfile, UCAN_AUTH_KIND,
    UCAN_OBSERVATION_OPERATION_PROFILE,
};
pub use request::{HttpRequest, map_live_request, map_view_request, parse_request, read_request};
pub use response::{ErrorBody, encode_view, error_response};
pub use server::{
    MemoryObservationSource, ObservationClock, ObservationSnapshot, ObservationSource, SystemClock,
    load_tls_config, serve, serve_on,
};
pub use ucan_auth::{ucan_operation_profile_id, ucan_request_binding, verify_ucan_header};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserveError {
    pub code: &'static str,
    pub message: String,
}

impl ObserveError {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for ObserveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ObserveError {}

pub type ObserveResult<T> = Result<T, ObserveError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpError {
    pub status: u16,
    pub code: &'static str,
    pub message: &'static str,
}

impl HttpError {
    #[must_use]
    pub const fn new(status: u16, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }
}
