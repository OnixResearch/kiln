//! Minimal HTTP/1.1 JSON client for the CLI and the MCP server. They reach
//! Kiln only through the fleet API.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::Value;

pub const CONNECT_TIMEOUT_MS: u64 = 5_000;
pub const RESPONSE_TIMEOUT_MS: u64 = 60_000;
pub const MAX_RESPONSE_BYTES: u64 = 4 * 1_024 * 1_024;
/// Environment variable naming the default server.
pub const SERVER_ENVIRONMENT: &str = "KILN_FLEET_SERVER";
const SCHEME: &str = "http://";

/// One API endpoint, `http://host:port`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiClient {
    authority: String,
}

impl ApiClient {
    /// # Errors
    /// Refuses anything but `http://host:port` with an optional trailing `/`.
    pub fn new(server: &str) -> Result<Self, String> {
        let authority = server
            .strip_prefix(SCHEME)
            .ok_or("server must be an http:// URL")?
            .trim_end_matches('/');
        if authority.is_empty()
            || authority.contains('/')
            || authority.contains('@')
            || authority
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ')
            || authority
                .rsplit_once(':')
                .is_none_or(|(_, port)| port.parse::<u16>().is_err())
        {
            return Err("server must be http://host:port".to_owned());
        }
        Ok(Self {
            authority: authority.to_owned(),
        })
    }

    /// Send one request and return the status and JSON body.
    ///
    /// # Errors
    /// Returns a diagnostic for connection failures and malformed responses;
    /// an HTTP error status is a successful exchange.
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        let address = self
            .authority
            .to_socket_addrs()
            .map_err(|error| format!("cannot resolve {}: {error}", self.authority))?
            .next()
            .ok_or_else(|| format!("cannot resolve {}", self.authority))?;
        let mut stream =
            TcpStream::connect_timeout(&address, Duration::from_millis(CONNECT_TIMEOUT_MS))
                .map_err(|error| format!("cannot reach {}: {error}", self.authority))?;
        let timeout = Some(Duration::from_millis(RESPONSE_TIMEOUT_MS));
        stream
            .set_read_timeout(timeout)
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(timeout)
            .map_err(|error| error.to_string())?;
        let payload = match body {
            Some(body) => serde_json::to_vec(body).map_err(|error| error.to_string())?,
            None => Vec::new(),
        };
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n",
            self.authority
        );
        if body.is_some() || method == "POST" {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                payload.len()
            ));
        }
        head.push_str("\r\n");
        stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(&payload))
            .and_then(|()| stream.flush())
            .map_err(|error| format!("cannot send the request: {error}"))?;
        let mut response = Vec::new();
        stream
            .take(MAX_RESPONSE_BYTES.saturating_add(1))
            .read_to_end(&mut response)
            .map_err(|error| format!("cannot read the response: {error}"))?;
        if u64::try_from(response.len()).unwrap_or(u64::MAX) > MAX_RESPONSE_BYTES {
            return Err("response exceeds its bound".to_owned());
        }
        parse_response(&response)
    }
}

fn parse_response(bytes: &[u8]) -> Result<(u16, Value), String> {
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("response has no header terminator")?;
    let head = std::str::from_utf8(bytes.get(..split).unwrap_or_default())
        .map_err(|_| "response head is not UTF-8")?;
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or("response has no status")?;
    let body = bytes.get(split.saturating_add(4)..).unwrap_or_default();
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).map_err(|_| "response body is not JSON")?
    };
    Ok((status, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_must_be_plain_http_authorities() {
        assert!(ApiClient::new("http://127.0.0.1:8095").is_ok());
        assert!(ApiClient::new("http://[::1]:8095/").is_ok());
        assert!(ApiClient::new("http://aspen3.example.ts.net:8095").is_ok());
        for bad in [
            "https://x:1",
            "http://x",
            "http://x:1/v1",
            "127.0.0.1:8095",
            "http://u@x:1",
        ] {
            assert!(ApiClient::new(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_response(b"HTTP/1.1 409 Conflict\r\nA: b\r\n\r\n{\"x\":1}"),
            Ok((409, serde_json::json!({"x": 1})))
        );
        assert!(parse_response(b"garbage").is_err());
    }
}
