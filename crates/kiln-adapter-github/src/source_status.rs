//! Idempotent source-issue closure after verified integration.
//!
//! Only the exact GitHub issue bound by an admitted source-status plan is
//! mutated. The client reads first, closes when open, and re-reads; it never
//! comments, reopens, edits other fields or mutates pull requests.

use std::io::Read;
use std::time::Duration;

use kiln_core::implementation::SourceStatusPlan;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::Deserialize;

use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

pub struct GithubSourceStatusClient {
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubSourceStatusClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubSourceStatusClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl GithubSourceStatusClient {
    pub fn new(token: ResolvedSecret, timeout_ms: u64) -> PortResult<Self> {
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(test)]
    fn new_fixture(token: ResolvedSecret, timeout_ms: u64, endpoint: &str) -> PortResult<Self> {
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_source_endpoint",
                "source.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_source_endpoint",
                "source.endpoint",
                "fixture endpoint must declare a port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_source_endpoint",
                "source.endpoint",
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
                "github_source_timeout",
                "source.timeout_ms",
                "source timeout must be positive",
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

    /// Read the current exact source issue state without mutation.
    // r[impl kiln.github.source_status]
    pub fn observe(&self, plan: &SourceStatusPlan) -> PortResult<String> {
        let (owner, repository, number) = issue_parts(plan)?;
        let issue: RawIssue = self.get(&format!(
            "{}/repos/{owner}/{repository}/issues/{number}",
            self.endpoint
        ))?;
        if !matches!(issue.state.as_str(), "open" | "closed") {
            return Err(domain(
                "github_source_state",
                "source.state",
                "source issue state is unsupported",
            ));
        }
        Ok(issue.state)
    }

    /// Close the exact source issue if it is open; re-read the final state.
    // r[impl kiln.github.source_status]
    pub fn close(&self, plan: &SourceStatusPlan) -> PortResult<String> {
        let (owner, repository, number) = issue_parts(plan)?;
        let url = format!(
            "{}/repos/{owner}/{repository}/issues/{number}",
            self.endpoint
        );
        let current: RawIssue = self.get(&url)?;
        if current.state == "closed" {
            return Ok(current.state);
        }
        if current.state != "open" {
            return Err(domain(
                "github_source_state",
                "source.state",
                "source issue state is unsupported",
            ));
        }
        let body = serde_json::json!({"state": "closed"}).to_string();
        let closed: RawIssue = self.patch(&url, &body)?;
        if closed.state != "closed" {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_source_close_unknown".to_string(),
                message: "source close response did not observe a closed issue".to_string(),
            }));
        }
        let observed: RawIssue = self.get(&url)?;
        if observed.state != "closed" {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_source_close_unknown".to_string(),
                message: "source close read-back did not observe a closed issue".to_string(),
            }));
        }
        Ok(observed.state)
    }

    fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> PortResult<T> {
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
            Ok(response) => self.decode(response),
            Err(ureq::Error::Status(401 | 403, _)) => Err(domain(
                "github_source_unauthorized",
                "source.http",
                "GitHub source credentials were refused",
            )),
            Err(ureq::Error::Status(404, _)) => Err(domain(
                "github_source_not_found",
                "source.http",
                "GitHub source issue was not found",
            )),
            Err(ureq::Error::Status(status, _)) => Err(infrastructure(
                "github_source_status",
                &format!("GitHub source returned HTTP {status}"),
            )),
            Err(_) => Err(infrastructure(
                "github_source_transport",
                "GitHub source transport failed",
            )),
        }
    }

    fn patch<T: serde::de::DeserializeOwned>(&self, url: &str, body: &str) -> PortResult<T> {
        let token = self.token.as_str()?;
        match self
            .agent
            .patch(url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("Content-Type", "application/json")
            .set("User-Agent", "kiln")
            .send_string(body)
        {
            Ok(response) => self.decode(response),
            Err(ureq::Error::Status(401 | 403, _)) => Err(domain(
                "github_source_unauthorized",
                "source.http",
                "GitHub source credentials were refused",
            )),
            Err(ureq::Error::Status(404, _)) => Err(domain(
                "github_source_not_found",
                "source.http",
                "GitHub source issue was not found",
            )),
            Err(ureq::Error::Status(422 | 409, _)) => {
                Err(PortFailure::Unknown(InfrastructureFailure {
                    code: "github_source_close_conflict".to_string(),
                    message: "source close conflicted and needs reconciliation".to_string(),
                }))
            }
            Err(ureq::Error::Status(status, _)) => Err(infrastructure(
                "github_source_status",
                &format!("GitHub source returned HTTP {status}"),
            )),
            Err(_) => Err(infrastructure(
                "github_source_transport",
                "GitHub source transport failed",
            )),
        }
    }

    fn decode<T: serde::de::DeserializeOwned>(&self, response: ureq::Response) -> PortResult<T> {
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| infrastructure("github_source_body", "source body could not be read"))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(domain(
                "github_source_bound",
                "source.response",
                "GitHub source response exceeds its byte bound",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_source_payload",
                "source.response",
                "GitHub source response is malformed",
            )
        })
    }
}

fn issue_parts(plan: &SourceStatusPlan) -> PortResult<(&str, &str, u64)> {
    if plan.source.source.provider_namespace.as_str() != "github"
        || plan.source.source.native_kind.as_str() != "issue"
    {
        return Err(domain(
            "github_source_namespace",
            "source.namespace",
            "source is not a GitHub issue reference",
        ));
    }
    let (repository, number) = plan.source.native_id.split_once('#').ok_or_else(|| {
        domain(
            "github_source_identity",
            "source.native_id",
            "source issue identity is malformed",
        )
    })?;
    let (owner, name) = repository.split_once('/').ok_or_else(|| {
        domain(
            "github_source_identity",
            "source.native_id",
            "source repository is malformed",
        )
    })?;
    let valid = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    let number = number.parse::<u64>().map_err(|_| {
        domain(
            "github_source_identity",
            "source.native_id",
            "source issue number is malformed",
        )
    })?;
    if !valid(owner) || !valid(name) || number == 0 {
        return Err(domain(
            "github_source_identity",
            "source.native_id",
            "source issue identity is malformed",
        ));
    }
    Ok((owner, name, number))
}

#[derive(Deserialize)]
struct RawIssue {
    state: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::SourceAction;
    use kiln_core::types::ContentId;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn token() -> ResolvedSecret {
        ResolvedSecret::from_bytes(b"fixture-token".to_vec()).unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> SourceStatusPlan {
        SourceStatusPlan {
            schema: kiln_core::implementation::SOURCE_STATUS_SCHEMA.to_string(),
            work_identity: ContentId::from_blake3_hex("1".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            work_revision: 1,
            attempt: ContentId::from_blake3_hex("2".repeat(64)).unwrap_or_else(|_| unreachable!()),
            fence: 3,
            integration: ContentId::from_blake3_hex("3".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            source: kiln_core::implementation::SourceRef {
                kind: kiln_core::implementation::WorkSourceKind::GitHubIssue,
                source: kiln_core::types::ExternalRef::new(
                    "github",
                    "issue",
                    "octocat/hello-world#7",
                )
                .unwrap_or_else(|_| unreachable!()),
                native_id: "octocat/hello-world#7".to_string(),
                version: ContentId::from_blake3_hex("4".repeat(64))
                    .unwrap_or_else(|_| unreachable!()),
            },
            action: SourceAction::Close,
        }
    }

    fn serve(
        responses: Vec<(&'static str, String)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in responses {
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
                requests.push(String::from_utf8_lossy(&request).into_owned());
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream
                    .write_all(head.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
                stream
                    .write_all(body.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
            }
            requests
        });
        (format!("http://{address}"), handle)
    }

    #[test]
    // r[verify kiln.github.source_status] r[verify kiln.github.source_status.scenario.close]
    fn closes_an_open_issue_and_reads_back_closed() {
        let (endpoint, server) = serve(vec![
            ("200 OK", "{\"state\":\"open\"}".to_string()),
            ("200 OK", "{\"state\":\"closed\"}".to_string()),
            ("200 OK", "{\"state\":\"closed\"}".to_string()),
        ]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            client
                .close(&plan())
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            "closed"
        );
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(
            requests.first().is_some_and(
                |request| request.starts_with("GET /repos/octocat/hello-world/issues/7")
            )
        );
        assert!(requests.get(1).is_some_and(|request| {
            request.starts_with("PATCH /repos/octocat/hello-world/issues/7")
        }));
    }

    #[test]
    // r[verify kiln.github.source_status]
    fn observes_source_state_without_mutation() {
        let (endpoint, server) = serve(vec![("200 OK", "{\"state\":\"open\"}".to_string())]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            client
                .observe(&plan())
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            "open"
        );
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(requests.len(), 1);
        assert!(
            requests
                .first()
                .is_some_and(|request| request.starts_with("GET "))
        );
    }

    #[test]
    // r[verify kiln.github.source_status]
    fn observes_a_closed_source_without_mutation() {
        let (endpoint, server) = serve(vec![("200 OK", "{\"state\":\"closed\"}".to_string())]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            client
                .observe(&plan())
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            "closed"
        );
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(requests.len(), 1);
    }

    #[test]
    // r[verify kiln.github.source_status] r[verify kiln.github.source_status.scenario.closed]
    fn already_closed_issue_is_idempotent() {
        let (endpoint, server) = serve(vec![("200 OK", "{\"state\":\"closed\"}".to_string())]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            client
                .close(&plan())
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            "closed"
        );
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(requests.len(), 1);
    }
}
