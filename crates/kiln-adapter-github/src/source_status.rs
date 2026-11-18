//! Read-only source-issue reconciliation after exact verified integration.
//!
//! GitHub's ordinary issue PATCH has no qualified source-version CAS. It must
//! not close a concurrently edited/reopened issue from stale admission. Observe
//! externally closed state; leave an open source for operator reconciliation.

use std::io::Read;
use std::time::Duration;

use kiln_core::implementation::SourceStatusPlan;
use kiln_provider_api::PortResult;
use serde::Deserialize;

use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubSourceObservation {
    pub state: String,
    pub revision: u64,
}

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
        reject_pull_request(&issue)?;
        if !matches!(issue.state.as_str(), "open" | "closed") {
            return Err(domain(
                "github_source_state",
                "source.state",
                "source issue state is unsupported",
            ));
        }
        Ok(issue.state)
    }

    /// Observe an explicit provider update sequence for the durable receipt.
    /// This does not pretend that REST bytes equal an admitted webhook hash.
    pub fn observe_exact(&self, plan: &SourceStatusPlan) -> PortResult<GithubSourceObservation> {
        let (owner, repository, number) = issue_parts(plan)?;
        let issue: RawIssue = self.get(&format!(
            "{}/repos/{owner}/{repository}/issues/{number}",
            self.endpoint
        ))?;
        reject_pull_request(&issue)?;
        let revision = issue
            .updated_at
            .as_deref()
            .and_then(crate::implementation::timestamp_millis)
            .ok_or_else(|| {
                domain(
                    "github_source_version_unknown",
                    "source.updated_at",
                    "source observation has no exact update sequence",
                )
            })?;
        if revision != plan.expected_observed_revision
            || !matches!(issue.state.as_str(), "open" | "closed")
        {
            return Err(domain(
                "github_source_version_stale",
                "source.updated_at",
                "source observation is stale or malformed",
            ));
        }
        Ok(GithubSourceObservation {
            state: issue.state,
            revision,
        })
    }

    /// Reconcile an externally closed issue. No PATCH is issued: read-then-close
    /// would race source edits and the raw-webhook source hash is not a REST
    /// issue version. The current work ledger remains the outer authority.
    // r[impl kiln.github.source_status]
    pub fn close(&self, plan: &SourceStatusPlan) -> PortResult<String> {
        let state = self.observe_exact(plan)?.state;
        if state != "closed" {
            return Err(domain(
                "github_source_conditional_update_unqualified",
                "source.version",
                "source is open; automatic closure lacks a qualified source-version guard",
            ));
        }
        Ok(state)
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

fn reject_pull_request(issue: &RawIssue) -> PortResult<()> {
    if issue.pull_request.is_some() {
        return Err(domain(
            "github_source_pull_request",
            "source.native_kind",
            "source issue number identifies a pull request",
        ));
    }
    Ok(())
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
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
    #[serde(default)]
    updated_at: Option<String>,
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
            expected_observed_revision: crate::implementation::timestamp_millis(
                "2026-11-13T00:00:00Z",
            )
            .unwrap_or_else(|| unreachable!()),
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
    // r[verify kiln.publication.atomicity] r[verify kiln.github.source_status.scenario.open]
    fn refuses_open_source_without_racing_a_patch() {
        let (endpoint, server) = serve(vec![(
            "200 OK",
            "{\"state\":\"open\",\"updated_at\":\"2026-11-13T00:00:00Z\"}".to_string(),
        )]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.close(&plan()),
            Err(kiln_provider_api::PortFailure::Domain(diagnostic))
                if diagnostic.code == "github_source_conditional_update_unqualified"
        ));
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(requests.len(), 1);
        assert!(requests.iter().all(|request| request.starts_with("GET ")));
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
    fn refuses_to_observe_a_pull_request_number() {
        let (endpoint, server) = serve(vec![(
            "200 OK",
            serde_json::json!({"state": "open", "pull_request": {}}).to_string(),
        )]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.observe(&plan()).is_err());
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
    fn refuses_to_close_a_pull_request_number() {
        let (endpoint, server) = serve(vec![(
            "200 OK",
            serde_json::json!({"state": "open", "pull_request": {}}).to_string(),
        )]);
        let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.close(&plan()).is_err());
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
        let (endpoint, server) = serve(vec![(
            "200 OK",
            "{\"state\":\"closed\",\"updated_at\":\"2026-11-13T00:00:00Z\"}".to_string(),
        )]);
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

    #[test]
    fn exact_source_observation_retains_version_and_refuses_stale_or_unknown() {
        let observed_at = "2026-11-13T00:00:00Z";
        let revision =
            crate::implementation::timestamp_millis(observed_at).unwrap_or_else(|| unreachable!());
        for (body, minimum, accepted) in [
            (
                serde_json::json!({"state":"closed", "updated_at":observed_at}),
                revision,
                true,
            ),
            (
                serde_json::json!({"state":"closed", "updated_at":observed_at}),
                revision + 1,
                false,
            ),
            (serde_json::json!({"state":"closed"}), revision, false),
        ] {
            let (endpoint, server) = serve(vec![("200 OK", body.to_string())]);
            let client = GithubSourceStatusClient::new_fixture(token(), 5000, &endpoint)
                .unwrap_or_else(|_| unreachable!());
            let mut plan = plan();
            plan.expected_observed_revision = minimum;
            let observed = client.observe_exact(&plan);
            assert_eq!(observed.is_ok(), accepted);
            if let Ok(observed) = observed {
                assert_eq!(observed.revision, revision);
            }
            assert!(
                server
                    .join()
                    .unwrap_or_else(|_| unreachable!())
                    .iter()
                    .all(|request| request.starts_with("GET "))
            );
        }
    }
}
