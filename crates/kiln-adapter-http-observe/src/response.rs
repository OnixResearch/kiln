use std::io::Write;

use kiln_observe_core::ObservationView;
use serde::Serialize;

use crate::profile::HttpObservationProfile;
use crate::{HttpError, ObserveError, ObserveResult};

pub const ERROR_SCHEMA: &str = "kiln.observe.http.error.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ErrorBody {
    pub schema: &'static str,
    pub code: &'static str,
    pub message: &'static str,
}

#[must_use]
pub fn error_response(error: HttpError) -> Vec<u8> {
    serde_json::to_vec(&ErrorBody {
        schema: ERROR_SCHEMA,
        code: error.code,
        message: error.message,
    })
    .unwrap_or_else(|_| {
        b"{\"schema\":\"kiln.observe.http.error.v1\",\"code\":\"internal\",\"message\":\"error response could not be encoded\"}"
            .to_vec()
    })
}

pub fn write_error<W: Write>(
    writer: &mut W,
    error: HttpError,
    max_bytes: usize,
) -> std::io::Result<()> {
    let body = error_response(error);
    if body.len() <= max_bytes {
        return write_response(writer, error.status, "application/json", &body, &[]);
    }
    let fallback = b"{\"code\":\"error\"}";
    if fallback.len() <= max_bytes {
        write_response(writer, error.status, "application/json", fallback, &[])
    } else {
        write_response(writer, error.status, "application/json", b"", &[])
    }
}

pub fn encode_view(view: &ObservationView, max_bytes: usize) -> ObserveResult<Vec<u8>> {
    let body = serde_json::to_vec(view)
        .map_err(|_| ObserveError::new("view_encoding", "observation view could not be encoded"))?;
    if body.len() > max_bytes {
        return Err(ObserveError::new(
            "view_encoding_bound",
            "encoded observation view exceeds its named byte bound",
        ));
    }
    Ok(body)
}

pub fn write_view<W: Write>(
    writer: &mut W,
    profile: &HttpObservationProfile,
    view: &ObservationView,
) -> ObserveResult<()> {
    let body = encode_view(
        view,
        usize::try_from(profile.content.max_view_bytes).unwrap_or(usize::MAX),
    )?;
    write_response(
        writer,
        200,
        &profile.content.view_content_type,
        &body,
        &[
            ("X-Kiln-View-Identity", view.identity.as_str()),
            ("X-Kiln-Observe-Profile", &profile.profile_id),
        ],
    )
    .map_err(|_| {
        ObserveError::new(
            "response_write",
            "observation response could not be written",
        )
    })
}

pub fn write_sse_headers<W: Write>(
    writer: &mut W,
    profile: &HttpObservationProfile,
) -> ObserveResult<()> {
    write!(
        writer,
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Kiln-Observe-Profile: {}\r\n\r\n",
        profile.content.event_content_type, profile.profile_id
    )
    .map_err(|_| ObserveError::new("response_write", "SSE headers could not be written"))?;
    writer
        .flush()
        .map_err(|_| ObserveError::new("response_write", "SSE headers could not be flushed"))
}

pub fn write_response<W: Write>(
    writer: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
    headers: &[(&str, &str)],
) -> std::io::Result<()> {
    write!(
        writer,
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        status_text(status),
        body.len()
    )?;
    for (name, value) in headers {
        write!(writer, "{name}: {value}\r\n")?;
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(body)?;
    writer.flush()
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    }
}
