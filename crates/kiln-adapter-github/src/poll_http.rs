//! Authenticated bounded GitHub issue polling over HTTPS.
//!
//! The transport performs one bounded GET per page against a fixed API origin,
//! validates the token and response headers, rejects pull requests from the
//! issues feed, and derives the next page from a bounded `Link` header. It does
//! not retry, schedule, admit or dispatch; callers use `poll_implementation_issues`.

use std::io::Read;
use std::time::Duration;

use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::Deserialize;

use crate::implementation::timestamp_millis;
use crate::poll::{IssuePollPage, IssuePollRecord, IssuePollRequest, IssuePollTransport};
use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_LINK_BYTES: usize = 2048;
const MAX_ACTOR_BYTES: usize = 128;

/// Fixed-origin GitHub issue polling transport.
pub struct GithubIssuePollClient {
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubIssuePollClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubIssuePollClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl GithubIssuePollClient {
    pub fn new(token: ResolvedSecret, timeout_ms: u64) -> PortResult<Self> {
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(test)]
    fn new_fixture(token: ResolvedSecret, timeout_ms: u64, endpoint: &str) -> PortResult<Self> {
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_poll_endpoint",
                "poll.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_poll_endpoint",
                "poll.endpoint",
                "fixture endpoint must declare a loopback port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_poll_endpoint",
                "poll.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            ));
        }
        Self::build(token, timeout_ms, endpoint.to_string(), false)
    }

    fn build(
        token: ResolvedSecret,
        timeout_ms: u64,
        endpoint: String,
        https: bool,
    ) -> PortResult<Self> {
        if timeout_ms == 0 {
            return Err(domain(
                "github_poll_timeout",
                "poll.timeout_ms",
                "poll timeout must be positive",
            ));
        }
        let timeout = Duration::from_millis(timeout_ms);
        let mut builder = ureq::AgentBuilder::new()
            .https_only(https)
            .redirects(0)
            .try_proxy_from_env(false)
            .max_idle_connections(0)
            .timeout(timeout)
            .timeout_connect(timeout);
        if https {
            let resolver = BoundedGithubResolver::current_executable(timeout_ms)?;
            builder = builder.resolver(move |netloc: &str| resolver.resolve(netloc));
        }
        Ok(Self {
            token,
            endpoint,
            agent: builder.build(),
        })
    }

    fn url(&self, request: &IssuePollRequest) -> PortResult<String> {
        if !identifier(&request.owner)
            || !identifier(&request.repository)
            || request.page == 0
            || request.per_page == 0
            || request.per_page > 100
        {
            return Err(domain(
                "github_poll_request",
                "poll.request",
                "poll request owner, repository, page or page size is invalid",
            ));
        }
        let mut url = format!(
            "{}/repos/{}/{}/issues?state=all&sort=updated&direction=asc&per_page={}&page={}",
            self.endpoint, request.owner, request.repository, request.per_page, request.page
        );
        if request.since_ms != 0 {
            url.push_str("&since=");
            url.push_str(&rfc3339(request.since_ms)?);
        }
        Ok(url)
    }

    fn get(&self, url: &str) -> PortResult<ureq::Response> {
        let token = self.token.as_str()?;
        match self
            .agent
            .get(url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("User-Agent", "kiln")
            .call()
        {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(401 | 403, _)) => Err(domain(
                "github_poll_unauthorized",
                "poll.http",
                "GitHub polling credentials were refused",
            )),
            Err(ureq::Error::Status(404, _)) => Err(domain(
                "github_poll_not_found",
                "poll.http",
                "GitHub repository was not found for polling",
            )),
            Err(ureq::Error::Status(429, _)) => Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_poll_rate_limited".to_string(),
                message: "GitHub polling was rate limited; outcome is unknown".to_string(),
            })),
            Err(ureq::Error::Status(status, _)) => Err(infrastructure(
                "github_poll_status",
                &format!("GitHub polling returned HTTP {status}"),
            )),
            Err(_) => Err(infrastructure(
                "github_poll_transport",
                "GitHub polling transport failed",
            )),
        }
    }
}

impl IssuePollTransport for GithubIssuePollClient {
    // r[impl kiln.implementation.polling] r[impl kiln.github.poll.transport] r[impl kiln.github.poll.transport.scenario.refused]
    fn list_issues(&mut self, request: &IssuePollRequest) -> PortResult<IssuePollPage> {
        let url = self.url(request)?;
        let response = self.get(&url)?;
        let next_page = next_page(response.header("link"), request.page)?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| {
                infrastructure(
                    "github_poll_body",
                    "GitHub polling response body could not be read",
                )
            })?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(domain(
                "github_poll_response_bound",
                "poll.response",
                "GitHub polling response exceeds its byte bound",
            ));
        }
        let raw: Vec<RawIssue> = serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_poll_payload",
                "poll.response",
                "GitHub polling response is not an issue array",
            )
        })?;
        if raw.len() > usize::from(request.per_page) {
            return Err(domain(
                "github_poll_page_bound",
                "poll.response",
                "GitHub polling page exceeds its requested size",
            ));
        }
        let mut records = Vec::with_capacity(raw.len());
        for issue in raw {
            if issue.pull_request.is_some() {
                continue;
            }
            let actor = issue.user.login;
            if issue.number == 0
                || actor.is_empty()
                || actor.len() > MAX_ACTOR_BYTES
                || actor.chars().any(char::is_control)
                || !matches!(issue.state.as_str(), "open" | "closed")
            {
                return Err(domain(
                    "github_poll_record",
                    "poll.record",
                    "GitHub polling record is malformed",
                ));
            }
            let updated_at_ms = timestamp_millis(&issue.updated_at).ok_or_else(|| {
                domain(
                    "github_poll_timestamp",
                    "poll.record.updated_at",
                    "GitHub polling record has an invalid timestamp",
                )
            })?;
            records.push(IssuePollRecord {
                number: issue.number,
                title: issue.title,
                body: issue.body,
                state: issue.state,
                updated_at_ms,
                actor,
            });
        }
        Ok(IssuePollPage { records, next_page })
    }
}

#[derive(Deserialize)]
struct RawIssue {
    number: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
    state: String,
    updated_at: String,
    user: RawUser,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawUser {
    login: String,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn next_page(link: Option<&str>, current: u32) -> PortResult<Option<u32>> {
    let Some(link) = link else {
        return Ok(None);
    };
    if link.len() > MAX_LINK_BYTES || link.chars().any(char::is_control) {
        return Err(domain(
            "github_poll_link",
            "poll.link",
            "GitHub polling Link header is malformed or oversized",
        ));
    }
    for part in link.split(',') {
        let part = part.trim();
        if !part.contains("rel=\"next\"") {
            continue;
        }
        let start = part
            .find("?page=")
            .map(|index| index + "?page=".len())
            .or_else(|| part.find("&page=").map(|index| index + "&page=".len()))
            .ok_or_else(|| {
                domain(
                    "github_poll_link",
                    "poll.link",
                    "GitHub polling next link has no page",
                )
            })?;
        let digits: String = part[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let page = digits.parse::<u32>().map_err(|_| {
            domain(
                "github_poll_link",
                "poll.link",
                "GitHub polling next page is invalid",
            )
        })?;
        if page <= current {
            return Err(domain(
                "github_poll_link",
                "poll.link",
                "GitHub polling next page does not advance",
            ));
        }
        return Ok(Some(page));
    }
    Ok(None)
}

fn rfc3339(ms: u64) -> PortResult<String> {
    let seconds = i64::try_from(ms / 1_000).map_err(|_| {
        domain(
            "github_poll_timestamp",
            "poll.since",
            "poll timestamp exceeds its supported range",
        )
    })?;
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    // Howard Hinnant's civil-from-days, admitted for 1970..9999.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    if !(1970..=9999).contains(&year) {
        return Err(domain(
            "github_poll_timestamp",
            "poll.since",
            "poll timestamp is outside the supported range",
        ));
    }
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn respond(
        status: &str,
        link: Option<&str>,
        body: &str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        listener
            .set_nonblocking(false)
            .unwrap_or_else(|_| unreachable!());
        let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let response = body.to_string();
        let status = status.to_string();
        let link = link.map(str::to_string);
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap_or_else(|_| unreachable!());
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap_or_else(|_| unreachable!());
                if read == 0 {
                    break;
                }
                request.extend_from_slice(buffer.get(..read).unwrap_or(&[]));
            }
            let mut head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                response.len()
            );
            if let Some(link) = link {
                head.push_str(&format!("Link: {link}\r\n"));
            }
            head.push_str("\r\n");
            head.push_str(&response);
            stream
                .write_all(head.as_bytes())
                .unwrap_or_else(|_| unreachable!());
            String::from_utf8_lossy(&request).into_owned()
        });
        (format!("http://{address}"), handle)
    }

    fn token() -> ResolvedSecret {
        ResolvedSecret::from_bytes(b"fixture-token".to_vec()).unwrap_or_else(|_| unreachable!())
    }

    fn request() -> IssuePollRequest {
        IssuePollRequest {
            owner: "octocat".to_string(),
            repository: "hello-world".to_string(),
            since_ms: 0,
            page: 1,
            per_page: 100,
        }
    }

    #[test]
    fn rfc3339_matches_the_github_timestamp_parser() {
        assert_eq!(
            rfc3339(0).unwrap_or_else(|_| unreachable!()),
            "1970-01-01T00:00:00Z"
        );
        let millis = timestamp_millis("2026-11-11T13:51:08Z").unwrap_or_else(|| unreachable!());
        assert_eq!(
            rfc3339(millis).unwrap_or_else(|_| unreachable!()),
            "2026-11-11T13:51:08Z"
        );
    }

    #[test]
    // r[verify kiln.github.poll.transport] r[verify kiln.github.poll.transport.scenario.pages]
    fn authenticated_polling_filters_pull_requests_and_reads_next_page() {
        let body = r#"[
            {"number":1,"title":"issue","body":"body","state":"open","updated_at":"2026-11-11T10:00:00Z","user":{"login":"operator"}},
            {"number":2,"title":"pr","state":"open","updated_at":"2026-11-11T10:01:00Z","user":{"login":"operator"},"pull_request":{}}
        ]"#;
        let (endpoint, handle) = respond(
            "200 OK",
            Some(
                "<https://api.github.com/repositories/1/issues?per_page=100&page=2>; rel=\"next\"",
            ),
            body,
        );
        let mut client = GithubIssuePollClient::new_fixture(token(), 5_000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let page = client
            .list_issues(&request())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(page.next_page, Some(2));
        assert_eq!(page.records.len(), 1);
        let record = page.records.first().unwrap_or_else(|| unreachable!());
        assert_eq!(record.number, 1);
        assert_eq!(record.actor, "operator");
        assert_eq!(record.state, "open");
        let request = handle.join().unwrap_or_else(|_| unreachable!());
        assert!(request.starts_with("GET /repos/octocat/hello-world/issues?"));
        assert!(request.contains("Authorization: Bearer fixture-token"));
        assert!(request.contains("per_page=100"));
        assert!(request.contains("page=1"));
    }

    #[test]
    // r[verify kiln.github.poll.transport.scenario.refused]
    fn refused_credentials_and_malformed_pages_fail() {
        let (endpoint, handle) = respond("401 Unauthorized", None, "{}");
        let mut client = GithubIssuePollClient::new_fixture(token(), 5_000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.list_issues(&request()).is_err());
        let _ = handle.join();

        let (endpoint, handle) = respond("200 OK", None, "not json");
        let mut client = GithubIssuePollClient::new_fixture(token(), 5_000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.list_issues(&request()).is_err());
        let _ = handle.join();
    }

    // r[verify kiln.github.poll.transport.scenario.bounds]
    #[test]
    fn next_page_rejects_regression_and_malformed_links() {
        assert_eq!(
            next_page(Some("<https://api.github.com/x?page=2>; rel=\"next\""), 1)
                .unwrap_or_else(|_| unreachable!()),
            Some(2)
        );
        assert!(next_page(Some("<https://api.github.com/x?page=1>; rel=\"next\""), 1).is_err());
        assert!(next_page(Some("<https://api.github.com/x>; rel=\"next\""), 1).is_err());
        assert!(next_page(Some(&"x".repeat(MAX_LINK_BYTES + 1)), 1).is_err());
    }
}
