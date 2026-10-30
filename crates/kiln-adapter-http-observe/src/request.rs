use std::collections::BTreeMap;
use std::io::{ErrorKind, Read};

use kiln_core::types::{AttemptId, RunId, StreamId};
use kiln_observe_core::{ObservationRequest, ViewRequest};

use crate::HttpError;
use crate::profile::HttpObservationProfile;

const MAX_QUERY_FIELDS: usize = 16;
/// Suggestion-anchor pages start at the first ranked anchor when no cursor is given.
const FIRST_ANCHOR_CURSOR: u64 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    pub target: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: BTreeMap<String, String>,
}

// r[impl kiln.observe.http.serve.scenario.unknown_kind]
pub fn read_request<R: Read>(
    reader: &mut R,
    profile: &HttpObservationProfile,
) -> Result<HttpRequest, HttpError> {
    let max_request = bound(profile.bounds.max_request_bytes);
    let max_header = bound(profile.bounds.max_header_bytes);
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1_024];
    loop {
        let count = reader
            .read(&mut chunk)
            .map_err(|error| match error.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                    HttpError::new(408, "request_timeout", "request read timed out")
                }
                _ => HttpError::new(400, "request_io", "request could not be read"),
            })?;
        if count == 0 {
            return Err(HttpError::new(
                400,
                "incomplete_request",
                "request ended before its header terminator",
            ));
        }
        let slice = chunk
            .get(..count)
            .ok_or_else(|| HttpError::new(400, "request_io", "request read exceeded its buffer"))?;
        bytes.extend_from_slice(slice);
        if bytes.len() > max_request {
            return Err(HttpError::new(
                413,
                "request_too_large",
                "request exceeds its named byte bound",
            ));
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_end = position.saturating_add(4);
            if header_end > max_header {
                return Err(HttpError::new(
                    431,
                    "header_too_large",
                    "request headers exceed their named byte bound",
                ));
            }
            if bytes.len() > header_end {
                return Err(HttpError::new(
                    400,
                    "unexpected_body",
                    "GET observation requests do not admit a body",
                ));
            }
            let header_bytes = bytes.get(..position).ok_or_else(|| {
                HttpError::new(400, "malformed_request", "request header range is invalid")
            })?;
            return parse_request(header_bytes, profile);
        }
    }
}

pub fn parse_request(
    header_bytes: &[u8],
    profile: &HttpObservationProfile,
) -> Result<HttpRequest, HttpError> {
    let text = std::str::from_utf8(header_bytes)
        .map_err(|_| HttpError::new(400, "malformed_request", "request headers are not UTF-8"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpError::new(400, "malformed_request", "request line is missing"))?;
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| HttpError::new(400, "malformed_request", "method is missing"))?;
    let target = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| HttpError::new(400, "malformed_request", "target is missing"))?;
    let version = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| HttpError::new(400, "malformed_request", "version is missing"))?;
    if parts.next().is_some() {
        return Err(HttpError::new(
            400,
            "malformed_request",
            "request line has extra fields",
        ));
    }
    if method != "GET" {
        return Err(HttpError::new(
            405,
            "method_not_allowed",
            "observation endpoints admit only GET",
        ));
    }
    if version != "HTTP/1.1" {
        return Err(HttpError::new(
            400,
            "unsupported_version",
            "observation endpoints admit only HTTP/1.1",
        ));
    }
    if !target.starts_with('/') || target.contains('#') {
        return Err(HttpError::new(
            400,
            "invalid_target",
            "request target must be an absolute path without a fragment",
        ));
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| HttpError::new(400, "malformed_header", "header line is malformed"))?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(HttpError::new(
                400,
                "malformed_header",
                "header name is empty",
            ));
        }
        if headers.insert(name, value.trim().to_string()).is_some() {
            return Err(HttpError::new(
                400,
                "duplicate_header",
                "header name is duplicated",
            ));
        }
        if headers.len() > bound(profile.bounds.max_header_fields) {
            return Err(HttpError::new(
                431,
                "too_many_headers",
                "request header count exceeds its named bound",
            ));
        }
    }
    if let Some(length) = headers.get("content-length")
        && length != "0"
    {
        return Err(HttpError::new(
            400,
            "unexpected_body",
            "GET observation requests do not admit a body",
        ));
    }
    if headers.contains_key("transfer-encoding") {
        return Err(HttpError::new(
            400,
            "unsupported_framing",
            "transfer encoding is not admitted",
        ));
    }
    if !headers.contains_key("host") {
        return Err(HttpError::new(
            400,
            "missing_host",
            "request host is required",
        ));
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), parse_query(query)?),
        None => (target.to_string(), Vec::new()),
    };
    Ok(HttpRequest {
        method: method.to_string(),
        target: target.to_string(),
        path,
        query,
        headers,
    })
}

fn parse_query(query: &str) -> Result<Vec<(String, String)>, HttpError> {
    if query.is_empty() {
        return Err(HttpError::new(
            400,
            "empty_query",
            "query separator requires at least one field",
        ));
    }
    let mut fields = Vec::new();
    for pair in query.split('&') {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| HttpError::new(400, "malformed_query", "query field is malformed"))?;
        if key.is_empty() || value.is_empty() || value.contains('%') {
            return Err(HttpError::new(
                400,
                "malformed_query",
                "query field is empty or uses unsupported encoding",
            ));
        }
        fields.push((key.to_string(), value.to_string()));
        if fields.len() > MAX_QUERY_FIELDS {
            return Err(HttpError::new(
                400,
                "too_many_query_fields",
                "query field count exceeds its named bound",
            ));
        }
    }
    Ok(fields)
}

pub fn admit_origin(
    request: &HttpRequest,
    profile: &HttpObservationProfile,
) -> Result<(), HttpError> {
    let host = request
        .headers
        .get("host")
        .ok_or_else(|| HttpError::new(400, "missing_host", "request host is required"))?;
    let expected = profile.host_header();
    let alternate = format!("{}:{}", profile.origin.host, profile.origin.port);
    let default_port = format!("{}:443", profile.origin.host);
    if host != &expected
        && host != &alternate
        && !(profile.origin.port == 443 && host == &default_port)
    {
        return Err(HttpError::new(
            400,
            "origin_mismatch",
            "request host does not match the registered origin",
        ));
    }
    Ok(())
}

// r[impl kiln.observe.http.serve]
pub fn map_view_request(
    request: &HttpRequest,
    state_revision: u64,
    profile: &HttpObservationProfile,
) -> Result<ObservationRequest, HttpError> {
    admit_origin(request, profile)?;
    let query = query_map(request)?;
    let view = if request.path == "/v1/views/state-summary" {
        require_no_query(&query)?;
        ViewRequest::StateSummary
    } else if request.path == "/v1/views/runs" {
        require_keys(&query, &["after", "limit"])?;
        let after = optional_id(&query, "after", run_id)?;
        let limit = limit(&query, profile)?;
        ViewRequest::RunList { after, limit }
    } else if let Some(id) = request.path.strip_prefix("/v1/views/runs/") {
        require_no_query(&query)?;
        ViewRequest::RunDetail {
            run_id: run_id(&id_segment(id)?)?,
        }
    } else if let Some(id) = request.path.strip_prefix("/v1/views/attempts/") {
        require_no_query(&query)?;
        ViewRequest::AttemptDetail {
            attempt_id: attempt_id(&id_segment(id)?)?,
        }
    } else if request.path == "/v1/views/events" {
        require_keys(&query, &["limit"])?;
        let limit = limit(&query, profile)?;
        ViewRequest::EventTail { limit }
    } else if let Some(id) = request.path.strip_prefix("/v1/views/logs/") {
        require_keys(&query, &["cursor", "max_bytes"])?;
        let stream_id = stream_id(&id_segment(id)?)?;
        let cursor = required_u64(&query, "cursor")?;
        let max_bytes =
            optional_u64(&query, "max_bytes")?.unwrap_or(profile.content.default_log_bytes);
        ViewRequest::LogRange {
            stream_id,
            cursor,
            max_bytes,
        }
    } else if request.path == "/v1/views/review-friction" {
        // r[impl kiln.observe.review.parity]
        require_no_query(&query)?;
        ViewRequest::ReviewFriction
    } else if request.path == "/v1/views/suggestion-anchors" {
        require_keys(&query, &["cursor", "limit"])?;
        let cursor = optional_u64(&query, "cursor")?.unwrap_or(FIRST_ANCHOR_CURSOR);
        let limit = limit(&query, profile)?;
        ViewRequest::SuggestionAnchors { cursor, limit }
    } else {
        return Err(HttpError::new(
            404,
            "unknown_view",
            "view kind is not in the observation vocabulary",
        ));
    };
    Ok(ObservationRequest {
        state_revision,
        view,
    })
}

pub fn map_live_request(
    request: &HttpRequest,
    profile: &HttpObservationProfile,
) -> Result<(Option<u64>, u64), HttpError> {
    admit_origin(request, profile)?;
    if request.path != "/v1/live" {
        return Err(HttpError::new(
            404,
            "unknown_view",
            "live endpoint is not in the observation vocabulary",
        ));
    }
    let query = query_map(request)?;
    require_keys(&query, &["after", "limit"])?;
    let after = optional_u64(&query, "after")?;
    let limit = limit(&query, profile)?;
    Ok((after, limit))
}

fn query_map(request: &HttpRequest) -> Result<BTreeMap<String, String>, HttpError> {
    let mut map = BTreeMap::new();
    for (key, value) in &request.query {
        if map.insert(key.clone(), value.clone()).is_some() {
            return Err(HttpError::new(
                400,
                "duplicate_query_field",
                "query field is duplicated",
            ));
        }
    }
    Ok(map)
}

fn require_keys(query: &BTreeMap<String, String>, allowed: &[&str]) -> Result<(), HttpError> {
    for key in query.keys() {
        if !allowed.iter().any(|allowed_key| key == allowed_key) {
            return Err(HttpError::new(
                400,
                "unexpected_query_field",
                "query field is not admitted by this view",
            ));
        }
    }
    Ok(())
}

fn require_no_query(query: &BTreeMap<String, String>) -> Result<(), HttpError> {
    if query.is_empty() {
        Ok(())
    } else {
        Err(HttpError::new(
            400,
            "unexpected_query",
            "view does not admit query fields",
        ))
    }
}

fn limit(
    query: &BTreeMap<String, String>,
    profile: &HttpObservationProfile,
) -> Result<u64, HttpError> {
    let limit = optional_u64(query, "limit")?.unwrap_or(profile.content.default_page_limit);
    if limit == 0 {
        return Err(HttpError::new(
            400,
            "invalid_limit",
            "page limit must be positive",
        ));
    }
    Ok(limit)
}

fn required_u64(query: &BTreeMap<String, String>, key: &'static str) -> Result<u64, HttpError> {
    optional_u64(query, key)?.ok_or_else(|| {
        HttpError::new(
            400,
            "missing_query_field",
            "required query field is missing",
        )
    })
}

fn optional_u64(
    query: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<u64>, HttpError> {
    match query.get(key) {
        Some(value) => value.parse::<u64>().map(Some).map_err(|_| {
            HttpError::new(
                400,
                "invalid_query_value",
                "query value is not an unsigned integer",
            )
        }),
        None => Ok(None),
    }
}

fn optional_id<T>(
    query: &BTreeMap<String, String>,
    key: &'static str,
    parse: fn(&str) -> Result<T, HttpError>,
) -> Result<Option<T>, HttpError> {
    match query.get(key) {
        Some(value) => parse(value).map(Some),
        None => Ok(None),
    }
}

fn id_segment(value: &str) -> Result<String, HttpError> {
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'%'))
    {
        return Err(HttpError::new(
            400,
            "invalid_identifier",
            "path identifier is empty or uses unsupported characters",
        ));
    }
    Ok(value.to_string())
}

fn run_id(value: &str) -> Result<RunId, HttpError> {
    RunId::new(value)
        .map_err(|_| HttpError::new(400, "invalid_run_id", "run identifier is malformed"))
}

fn attempt_id(value: &str) -> Result<AttemptId, HttpError> {
    AttemptId::new(value)
        .map_err(|_| HttpError::new(400, "invalid_attempt_id", "attempt identifier is malformed"))
}

fn stream_id(value: &str) -> Result<StreamId, HttpError> {
    StreamId::new(value)
        .map_err(|_| HttpError::new(400, "invalid_stream_id", "stream identifier is malformed"))
}

fn bound(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
