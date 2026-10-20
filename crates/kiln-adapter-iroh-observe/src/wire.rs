use kiln_observe_core::{ObservationRequest, ObservationView, ViewBody};
use serde::{Deserialize, Serialize};

use crate::{ObserveError, ObserveResult};

pub const REQUEST_SCHEMA: &str = "kiln.observe.iroh.request.v1";
pub const RESPONSE_SCHEMA: &str = "kiln.observe.iroh.response.v1";
pub const PAYLOAD_LOG_BYTES: &str = "log_bytes";
pub const PAYLOAD_EVENT_RECORDS: &str = "event_records";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohObserveRequest {
    pub schema: String,
    pub request: ObservationRequest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohObserveResponse {
    pub schema: String,
    pub result: IrohResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IrohResult {
    Ok {
        view: Box<ObservationView>,
        payload: Option<PayloadIdentity>,
    },
    Err {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadIdentity {
    pub kind: String,
    pub blake3: String,
}

impl IrohObserveResponse {
    pub fn ok(view: ObservationView) -> ObserveResult<Self> {
        let payload = payload_identity(&view)?;
        Ok(Self {
            schema: String::from(RESPONSE_SCHEMA),
            result: IrohResult::Ok {
                view: Box::new(view),
                payload,
            },
        })
    }

    #[must_use]
    pub fn error(code: &str, message: &str) -> Self {
        Self {
            schema: String::from(RESPONSE_SCHEMA),
            result: IrohResult::Err {
                code: code.to_string(),
                message: message.to_string(),
            },
        }
    }
}

// r[impl kiln.observe.iroh.verify]
// r[impl kiln.observe.iroh.verify.scenario.verified]
pub fn payload_identity(view: &ObservationView) -> ObserveResult<Option<PayloadIdentity>> {
    match &view.body {
        ViewBody::LogRange(range) => match &range.bytes {
            Some(bytes) => Ok(Some(PayloadIdentity {
                kind: String::from(PAYLOAD_LOG_BYTES),
                blake3: blake3_hex(bytes),
            })),
            None => Ok(None),
        },
        ViewBody::EventTail(tail) => {
            let bytes = serde_json::to_vec(&tail.records).map_err(|_| {
                ObserveError::new(
                    "iroh_payload_encoding",
                    "event-record payload could not be encoded",
                )
            })?;
            Ok(Some(PayloadIdentity {
                kind: String::from(PAYLOAD_EVENT_RECORDS),
                blake3: blake3_hex(&bytes),
            }))
        }
        _ => Ok(None),
    }
}

// r[impl kiln.observe.iroh.verify.scenario.corrupted]
pub fn verify_payload(
    view: &ObservationView,
    payload: Option<&PayloadIdentity>,
) -> ObserveResult<()> {
    let expected = payload_identity(view)?;
    match (expected.as_ref(), payload) {
        (None, None) => Ok(()),
        (Some(expected), Some(actual)) if expected == actual => Ok(()),
        (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => Err(ObserveError::new(
            "iroh_payload_identity",
            "payload identity does not match the served view",
        )),
    }
}

// r[impl kiln.observe.iroh.equiv]
// r[impl kiln.observe.iroh.equiv.scenario.equal_views]
// r[impl kiln.observe.iroh.equiv.scenario.divergence]
pub fn admit_equivalence(http: &ObservationView, iroh: &ObservationView) -> ObserveResult<()> {
    if http == iroh {
        Ok(())
    } else {
        Err(ObserveError::new(
            "iroh_view_divergence",
            "HTTP and iroh observation views differ",
        ))
    }
}

#[must_use]
pub fn logical_view(response: &IrohObserveResponse) -> Option<&ObservationView> {
    match &response.result {
        IrohResult::Ok { view, .. } => Some(view.as_ref()),
        IrohResult::Err { .. } => None,
    }
}

pub fn encode_request(request: &IrohObserveRequest, max_bytes: usize) -> ObserveResult<Vec<u8>> {
    encode_value(request, max_bytes)
}

pub fn decode_request(bytes: &[u8], max_bytes: usize) -> ObserveResult<IrohObserveRequest> {
    if bytes.len() > max_bytes {
        return Err(ObserveError::new(
            "iroh_frame_bound",
            "Iroh observation request exceeds its byte bound",
        ));
    }
    let request: IrohObserveRequest = serde_json::from_slice(bytes).map_err(|_| {
        ObserveError::new(
            "iroh_request_encoding",
            "Iroh observation request is malformed",
        )
    })?;
    if request.schema != REQUEST_SCHEMA {
        return Err(ObserveError::new(
            "iroh_request_schema",
            "Iroh observation request schema is unsupported",
        ));
    }
    Ok(request)
}

pub fn encode_response(response: &IrohObserveResponse, max_bytes: usize) -> ObserveResult<Vec<u8>> {
    encode_value(response, max_bytes)
}

pub fn decode_response(bytes: &[u8], max_bytes: usize) -> ObserveResult<IrohObserveResponse> {
    if bytes.len() > max_bytes {
        return Err(ObserveError::new(
            "iroh_frame_bound",
            "Iroh observation response exceeds its byte bound",
        ));
    }
    let response: IrohObserveResponse = serde_json::from_slice(bytes).map_err(|_| {
        ObserveError::new(
            "iroh_response_encoding",
            "Iroh observation response is malformed",
        )
    })?;
    if response.schema != RESPONSE_SCHEMA {
        return Err(ObserveError::new(
            "iroh_response_schema",
            "Iroh observation response schema is unsupported",
        ));
    }
    if let IrohResult::Ok { view, payload } = &response.result {
        verify_payload(view, payload.as_ref())?;
    }
    Ok(response)
}

fn encode_value<T: Serialize>(value: &T, max_bytes: usize) -> ObserveResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        ObserveError::new(
            "iroh_frame_encoding",
            "Iroh observation frame could not be encoded",
        )
    })?;
    if bytes.len() > max_bytes {
        return Err(ObserveError::new(
            "iroh_frame_bound",
            "Iroh observation frame exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
