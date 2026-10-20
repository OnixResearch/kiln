#![forbid(unsafe_code)]

// r[impl kiln.observe.iroh.profile]
mod conformance;
mod profile;
mod shell;
mod wire;

#[cfg(test)]
mod tests;

use std::fmt::{Display, Formatter};

pub use conformance::{
    IrohConformanceManifest, REQUIRED_NEGATIVE_CASES, REQUIRED_POSITIVE_CASES,
    admit_iroh_conformance,
};
pub use profile::{
    BAO_TREE_COHORT, CohortProfile, EndpointProfile, IROH_BLOBS_COHORT, IROH_OBSERVATION_ALPN,
    IROH_OBSERVATION_NON_CLAIMS, IROH_VERSION, IrohObservationProfile, PROFILE_SCHEMA, PeerProfile,
    did_key,
};
pub use shell::{bind_endpoint, client_exchange, remote_address, serve};
pub use wire::{
    IrohObserveRequest, IrohObserveResponse, IrohResult, PAYLOAD_EVENT_RECORDS, PAYLOAD_LOG_BYTES,
    PayloadIdentity, REQUEST_SCHEMA, RESPONSE_SCHEMA, admit_equivalence, decode_request,
    decode_response, encode_request, encode_response, logical_view, payload_identity,
    verify_payload,
};

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
