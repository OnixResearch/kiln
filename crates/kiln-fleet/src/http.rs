//! Bounded HTTP/1.1 request parsing and JSON responses.
//!
//! The server reads one request per connection and always closes it. Header
//! bytes, header count and body length have named bounds; a body needs
//! `Content-Length`, chunked transfer is refused, and nothing is read past
//! the declared length.

use std::io::{Read, Write};

use serde_json::Value;

/// Largest accepted JSON body.
pub const MAX_BODY_BYTES: usize = 64 * 1_024;
/// Largest request line plus headers.
pub const MAX_HEADER_BYTES: usize = 16 * 1_024;
pub const MAX_HEADERS: usize = 64;
const READ_CHUNK_BYTES: usize = 4_096;
const HEADER_END: &[u8] = b"\r\n\r\n";

/// One parsed request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The target path without any query.
    pub path: String,
    pub body: Vec<u8>,
}

/// One response: a status and a JSON body.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

impl Response {
    #[must_use]
    pub const fn new(status: u16, body: Value) -> Self {
        Self { status, body }
    }

    /// `{"error": message, "code": code}`.
    #[must_use]
    pub fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            body: serde_json::json!({ "error": message.into(), "code": code }),
        }
    }
}

/// A request the server refuses before routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpError {
    Malformed(&'static str),
    HeadersTooLarge,
    LengthRequired,
    PayloadTooLarge,
    Unsupported(&'static str),
    Closed,
}

impl HttpError {
    #[must_use]
    pub fn response(self) -> Response {
        match self {
            Self::Malformed(message) => Response::error(400, "http_malformed", message),
            Self::HeadersTooLarge => {
                Response::error(431, "http_headers", "request headers exceed their bound")
            }
            Self::LengthRequired => {
                Response::error(411, "http_length", "a request body needs Content-Length")
            }
            Self::PayloadTooLarge => Response::error(
                413,
                "http_body",
                format!("request body exceeds {MAX_BODY_BYTES} bytes"),
            ),
            Self::Unsupported(message) => Response::error(501, "http_unsupported", message),
            Self::Closed => Response::error(
                400,
                "http_closed",
                "connection closed before a full request",
            ),
        }
    }
}

/// Read one request from `reader`.
///
/// # Errors
/// Refuses malformed request lines and headers, oversized headers or bodies,
/// a body without `Content-Length`, conflicting lengths and chunked transfer.
pub fn read_request(reader: &mut impl Read) -> Result<Request, HttpError> {
    let mut buffer = Vec::new();
    let header_end = loop {
        if let Some(position) = find(&buffer, HEADER_END) {
            break position;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(HttpError::HeadersTooLarge);
        }
        let mut chunk = [0_u8; READ_CHUNK_BYTES];
        let read = reader.read(&mut chunk).map_err(|_| HttpError::Closed)?;
        if read == 0 {
            return Err(HttpError::Closed);
        }
        buffer.extend_from_slice(chunk.get(..read).ok_or(HttpError::Closed)?);
    };
    if header_end > MAX_HEADER_BYTES {
        return Err(HttpError::HeadersTooLarge);
    }
    let head = std::str::from_utf8(buffer.get(..header_end).ok_or(HttpError::Closed)?)
        .map_err(|_| HttpError::Malformed("request head is not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(HttpError::Malformed("missing request line"))?;
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HttpError::Malformed("malformed request line"));
    };
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(HttpError::Unsupported(
            "only HTTP/1.0 and HTTP/1.1 are served",
        ));
    }
    if method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(HttpError::Malformed("malformed method"));
    }
    if !target.starts_with('/') || target.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(HttpError::Malformed(
            "request target must be an origin path",
        ));
    }
    let path = target.split('?').next().unwrap_or(target).to_owned();
    let mut content_length: Option<usize> = None;
    let mut count = 0_usize;
    for line in lines {
        count = count.saturating_add(1);
        if count > MAX_HEADERS {
            return Err(HttpError::HeadersTooLarge);
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(HttpError::Malformed("malformed header"))?;
        if name.is_empty() || name.bytes().any(|byte| !byte.is_ascii_graphic()) {
            return Err(HttpError::Malformed("malformed header name"));
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(HttpError::Unsupported(
                "transfer encodings are not accepted",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            let length = value
                .parse::<usize>()
                .map_err(|_| HttpError::Malformed("Content-Length is not a number"))?;
            if content_length.is_some_and(|previous| previous != length) {
                return Err(HttpError::Malformed("conflicting Content-Length headers"));
            }
            content_length = Some(length);
        }
    }
    let body_start = header_end.saturating_add(HEADER_END.len());
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    let length = match content_length {
        Some(length) => length,
        None if method == "POST" || method == "PUT" || method == "PATCH" => {
            return Err(HttpError::LengthRequired);
        }
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return Err(HttpError::PayloadTooLarge);
    }
    if body.len() > length {
        return Err(HttpError::Malformed("body exceeds Content-Length"));
    }
    let remaining = length.saturating_sub(body.len());
    let mut rest = Vec::with_capacity(remaining);
    reader
        .take(u64::try_from(remaining).unwrap_or(u64::MAX))
        .read_to_end(&mut rest)
        .map_err(|_| HttpError::Closed)?;
    if rest.len() != remaining {
        return Err(HttpError::Closed);
    }
    body.extend_from_slice(&rest);
    Ok(Request {
        method: method.to_owned(),
        path,
        body,
    })
}

/// Write one response and close semantics (`Connection: close`).
///
/// # Errors
/// Returns the socket error.
pub fn write_response(writer: &mut impl Write, response: &Response) -> std::io::Result<()> {
    let body = serde_json::to_vec(&response.body).unwrap_or_else(|_| b"{}".to_vec());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        response.status,
        reason(response.status),
        body.len()
    )?;
    writer.write_all(&body)?;
    writer.flush()
}

#[must_use]
pub const fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> Result<Request, HttpError> {
        read_request(&mut std::io::Cursor::new(bytes.to_vec()))
    }

    #[test]
    // r[verify kiln.fleet.api.scenario.identity]
    fn bounded_requests_parse_and_oversized_ones_are_refused() {
        let request =
            parse(b"POST /v1/claims?x=1 HTTP/1.1\r\nHost: a\r\nContent-Length: 2\r\n\r\n{}")
                .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/claims");
        assert_eq!(request.body, b"{}");
        let get = parse(b"GET /v1/health HTTP/1.1\r\n\r\n").unwrap_or_else(|_| unreachable!());
        assert!(get.body.is_empty());

        let oversized = format!(
            "POST /v1/claims HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert_eq!(parse(oversized.as_bytes()), Err(HttpError::PayloadTooLarge));
        assert_eq!(
            parse(b"POST /v1/claims HTTP/1.1\r\n\r\n{}"),
            Err(HttpError::LengthRequired)
        );
        assert!(matches!(
            parse(b"POST /v1/claims HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"),
            Err(HttpError::Unsupported(_))
        ));
        assert!(matches!(
            parse(b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n"),
            Err(HttpError::Malformed(_))
        ));
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n{}"),
            Err(HttpError::Closed)
        );
        assert!(matches!(
            parse(b"GET http://x/ HTTP/1.1\r\n\r\n"),
            Err(HttpError::Malformed(_))
        ));
        assert!(matches!(
            parse(b"GET / HTTP/2\r\n\r\n"),
            Err(HttpError::Unsupported(_))
        ));
        let mut huge = b"GET / HTTP/1.1\r\n".to_vec();
        huge.extend(std::iter::repeat_n(b'a', MAX_HEADER_BYTES + 10));
        assert_eq!(parse(&huge), Err(HttpError::HeadersTooLarge));
        let many: String = (0..=MAX_HEADERS)
            .map(|index| format!("X-{index}: 1\r\n"))
            .collect();
        assert_eq!(
            parse(format!("GET / HTTP/1.1\r\n{many}\r\n").as_bytes()),
            Err(HttpError::HeadersTooLarge)
        );
    }

    #[test]
    fn responses_carry_length_and_close() {
        let mut out = Vec::new();
        write_response(&mut out, &Response::new(201, serde_json::json!({"a": 1})))
            .unwrap_or_else(|_| unreachable!());
        let text = String::from_utf8(out).unwrap_or_else(|_| unreachable!());
        assert!(text.starts_with("HTTP/1.1 201 Created\r\n"));
        assert!(text.contains("Content-Length: 7\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.ends_with("{\"a\":1}"));
    }
}
