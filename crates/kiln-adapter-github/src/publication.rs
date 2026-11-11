//! Idempotent verified-PR publication to GitHub.
//!
//! The caller supplies an admitted publication plan with a verified candidate.
//! Branch creation is reconciled before retry; PR creation reconciles exact
//! head/base matches and refuses ambiguity. No merge, force push, branch update
//! or source-status mutation is performed here.

use std::io::Read;
use std::time::Duration;

use kiln_core::implementation::PublicationPlan;
use kiln_core::types::ExternalRef;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::Deserialize;

use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationOutcome {
    pub branch_head: ExternalRef,
    pub pull_request: ExternalRef,
    pub number: u64,
    pub html_url: String,
    pub merged: bool,
}

pub struct GithubPublicationClient {
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubPublicationClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubPublicationClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl GithubPublicationClient {
    pub fn new(token: ResolvedSecret, timeout_ms: u64) -> PortResult<Self> {
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(test)]
    fn new_fixture(token: ResolvedSecret, timeout_ms: u64, endpoint: &str) -> PortResult<Self> {
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_publication_endpoint",
                "publication.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_publication_endpoint",
                "publication.endpoint",
                "fixture endpoint must declare a port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_publication_endpoint",
                "publication.endpoint",
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
                "github_publication_timeout",
                "publication.timeout_ms",
                "publication timeout must be positive",
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

    /// Ensure the exact candidate branch exists at the candidate commit.
    // r[impl kiln.github.publication]
    pub fn ensure_branch(&self, plan: &PublicationPlan) -> PortResult<ExternalRef> {
        self.ensure_branch_at(plan, 0)
    }

    fn ensure_branch_at(&self, plan: &PublicationPlan, retries: u8) -> PortResult<ExternalRef> {
        validate_plan(plan)?;
        let (owner, repository) = repository_parts(&plan.repository)?;
        let path = format!(
            "{}/repos/{owner}/{repository}/git/ref/heads/{}",
            self.endpoint,
            encode(&plan.branch)
        );
        match self.get(&path) {
            Ok(response) => {
                let raw: RawRef = self.decode(response)?;
                if raw.object.sha != plan.candidate.native_id.as_str() {
                    return Err(domain(
                        "github_publication_branch_conflict",
                        "publication.branch",
                        "existing branch head differs from the candidate",
                    ));
                }
                commit_ref(&raw.object.sha)
            }
            Err(HttpError::Status(404)) => self.create_branch(plan, retries),
            Err(error) => Err(error.into_failure()),
        }
    }

    fn create_branch(&self, plan: &PublicationPlan, retries: u8) -> PortResult<ExternalRef> {
        let (owner, repository) = repository_parts(&plan.repository)?;
        let body = serde_json::json!({
            "ref": format!("refs/heads/{}", plan.branch),
            "sha": plan.candidate.native_id.as_str(),
        })
        .to_string();
        match self.post(
            &format!("{}/repos/{owner}/{repository}/git/refs", self.endpoint),
            &body,
        ) {
            Ok(response) => {
                let raw: RawRef = self.decode(response)?;
                if raw.object.sha != plan.candidate.native_id.as_str() {
                    return Err(infrastructure(
                        "github_publication_branch",
                        "GitHub created a branch at a different head",
                    ));
                }
                commit_ref(&raw.object.sha)
            }
            Err(HttpError::Status(422 | 409)) if retries == 0 => self.ensure_branch_at(plan, 1),
            Err(HttpError::Status(422 | 409)) => Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_publication_branch_race".to_string(),
                message: "branch creation raced and remained unreconciled".to_string(),
            })),
            Err(error) => Err(error.into_failure()),
        }
    }

    /// Ensure exactly one open PR exists for the candidate branch and base.
    // r[impl kiln.github.publication]
    pub fn ensure_pull_request(&self, plan: &PublicationPlan) -> PortResult<PublicationOutcome> {
        self.ensure_pull_request_at(plan, 0)
    }

    fn ensure_pull_request_at(
        &self,
        plan: &PublicationPlan,
        retries: u8,
    ) -> PortResult<PublicationOutcome> {
        validate_plan(plan)?;
        let (owner, repository) = repository_parts(&plan.repository)?;
        let list = format!(
            "{}/repos/{owner}/{repository}/pulls?state=open&head={}:{}&base={}&per_page=100",
            self.endpoint,
            encode(owner),
            encode(&plan.branch),
            encode(&plan.base_branch)
        );
        let response = self.get(&list).map_err(HttpError::into_failure)?;
        if has_next_link(response.header("link"))? {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_publication_pr_pagination".to_string(),
                message: "pull request reconciliation is incomplete across pages".to_string(),
            }));
        }
        let mut pulls: Vec<RawPull> = self.decode(response)?;
        pulls.retain(|pull| pull.head.r#ref == plan.branch && pull.base.r#ref == plan.base_branch);
        let mut open: Vec<&RawPull> = pulls
            .iter()
            .filter(|pull| pull.state == "open" && !pull.merged)
            .collect();
        let mut merged: Vec<&RawPull> = pulls.iter().filter(|pull| pull.merged).collect();
        if open.len() > 1 || merged.len() > 1 {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_publication_ambiguous".to_string(),
                message: "more than one matching pull request exists".to_string(),
            }));
        }
        if let Some(pull) = open.pop().or_else(|| merged.pop()) {
            return outcome(plan, pull);
        }
        if pulls.iter().any(|pull| pull.state == "closed") {
            return Err(domain(
                "github_publication_pr_closed",
                "publication.pull_request",
                "an unmerged closed pull request already exists for this candidate",
            ));
        }
        let body = serde_json::json!({
            "title": plan.title,
            "body": plan.body,
            "head": plan.branch,
            "base": plan.base_branch,
        })
        .to_string();
        match self.post(
            &format!("{}/repos/{owner}/{repository}/pulls", self.endpoint),
            &body,
        ) {
            Ok(response) => {
                let pull: RawPull = self.decode(response)?;
                outcome(plan, &pull)
            }
            Err(HttpError::Status(422 | 409)) if retries == 0 => {
                self.ensure_pull_request_at(plan, 1)
            }
            Err(HttpError::Status(422 | 409)) => Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_publication_pr_race".to_string(),
                message: "pull request creation raced and remained unreconciled".to_string(),
            })),
            Err(error) => Err(error.into_failure()),
        }
    }

    fn get(&self, url: &str) -> Result<ureq::Response, HttpError> {
        let token = self.token.as_str().map_err(HttpError::Other)?;
        self.agent
            .get(url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("User-Agent", "kiln")
            .call()
            .map_err(HttpError::from_ureq)
    }

    fn post(&self, url: &str, body: &str) -> Result<ureq::Response, HttpError> {
        let token = self.token.as_str().map_err(HttpError::Other)?;
        self.agent
            .post(url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("Content-Type", "application/json")
            .set("User-Agent", "kiln")
            .send_string(body)
            .map_err(HttpError::from_ureq)
    }

    fn decode<T: serde::de::DeserializeOwned>(&self, response: ureq::Response) -> PortResult<T> {
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| infrastructure("github_publication_body", "body could not be read"))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(domain(
                "github_publication_bound",
                "publication.response",
                "GitHub publication response exceeds its byte bound",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_publication_payload",
                "publication.response",
                "GitHub publication response is malformed",
            )
        })
    }
}

fn validate_plan(plan: &PublicationPlan) -> PortResult<()> {
    let safe_branch = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains("..")
            && !value.chars().any(char::is_control)
    };
    let commit = plan.candidate.native_id.as_str();
    if plan.schema != kiln_core::implementation::PUBLICATION_SCHEMA
        || !safe_branch(&plan.branch)
        || !safe_branch(&plan.base_branch)
        || plan.title.trim().is_empty()
        || plan.title.len() > 256
        || plan.body.len() > 16_384
        || !matches!(commit.len(), 40 | 64)
        || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(domain(
            "github_publication_plan",
            "publication.plan",
            "publication plan has malformed branch, commit or text",
        ));
    }
    Ok(())
}

fn repository_parts(repository: &str) -> PortResult<(&str, &str)> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err(domain(
            "github_publication_repository",
            "publication.repository",
            "publication repository must be exactly owner/name",
        ));
    };
    let valid = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if !valid(owner) || !valid(name) || name.contains('/') {
        return Err(domain(
            "github_publication_repository",
            "publication.repository",
            "publication repository identifiers are malformed",
        ));
    }
    Ok((owner, name))
}

fn has_next_link(link: Option<&str>) -> PortResult<bool> {
    let Some(link) = link else {
        return Ok(false);
    };
    if link.len() > 2048 || link.chars().any(char::is_control) {
        return Err(domain(
            "github_publication_link",
            "publication.link",
            "GitHub publication Link header is malformed or oversized",
        ));
    }
    if link.contains("rel=\"next\"") {
        return Ok(true);
    }
    Ok(false)
}

fn outcome(plan: &PublicationPlan, pull: &RawPull) -> PortResult<PublicationOutcome> {
    if pull.head.sha != plan.candidate.native_id.as_str()
        || pull.base.r#ref != plan.base_branch
        || !(pull.state == "open" || (pull.state == "closed" && pull.merged))
    {
        return Err(domain(
            "github_publication_pr_conflict",
            "publication.pull_request",
            "existing pull request head/base/state differs from the candidate",
        ));
    }
    Ok(PublicationOutcome {
        branch_head: commit_ref(plan.candidate.native_id.as_str())?,
        pull_request: ExternalRef::new(
            "github",
            "pull_request",
            &format!("{}#{}", plan.repository, pull.number),
        )?,
        number: pull.number,
        html_url: pull.html_url.clone(),
        merged: pull.merged,
    })
}

fn commit_ref(sha: &str) -> PortResult<ExternalRef> {
    ExternalRef::new("git", "commit", sha).map_err(|_| {
        infrastructure(
            "github_publication_ref",
            "candidate commit identity is not a bounded external reference",
        )
    })
}

fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

enum HttpError {
    Status(u16),
    Other(PortFailure),
}

impl HttpError {
    fn from_ureq(error: ureq::Error) -> Self {
        match error {
            ureq::Error::Status(status, _) => Self::Status(status),
            _ => Self::Other(infrastructure(
                "github_publication_transport",
                "GitHub publication transport failed",
            )),
        }
    }

    fn into_failure(self) -> PortFailure {
        match self {
            Self::Status(401 | 403) => domain(
                "github_publication_unauthorized",
                "publication.http",
                "GitHub publication credentials were refused",
            ),
            Self::Status(404) => domain(
                "github_publication_not_found",
                "publication.http",
                "GitHub publication target was not found",
            ),
            Self::Status(status) => infrastructure(
                "github_publication_status",
                &format!("GitHub publication returned HTTP {status}"),
            ),
            Self::Other(error) => error,
        }
    }
}

#[derive(Deserialize)]
struct RawRef {
    object: RawObject,
}
#[derive(Deserialize)]
struct RawObject {
    sha: String,
}
#[derive(Deserialize)]
struct RawPull {
    number: u64,
    state: String,
    merged: bool,
    html_url: String,
    head: RawHead,
    base: RawBase,
}
#[derive(Deserialize)]
struct RawHead {
    #[serde(rename = "ref")]
    r#ref: String,
    sha: String,
}
#[derive(Deserialize)]
struct RawBase {
    #[serde(rename = "ref")]
    r#ref: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::types::ContentId;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn token() -> ResolvedSecret {
        ResolvedSecret::from_bytes(b"fixture-token".to_vec()).unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> PublicationPlan {
        PublicationPlan {
            schema: kiln_core::implementation::PUBLICATION_SCHEMA.to_string(),
            work_identity: ContentId::from_blake3_hex("1".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            work_revision: 1,
            attempt: ContentId::from_blake3_hex("2".repeat(64)).unwrap_or_else(|_| unreachable!()),
            fence: 3,
            handoff: ContentId::from_blake3_hex("3".repeat(64)).unwrap_or_else(|_| unreachable!()),
            verification: ContentId::from_blake3_hex("4".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            repository: "octocat/hello-world".to_string(),
            base: ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
            base_branch: "main".to_string(),
            candidate: ExternalRef::new("git", "commit", &"a".repeat(40))
                .unwrap_or_else(|_| unreachable!()),
            tree: ExternalRef::new("git", "tree", "tree").unwrap_or_else(|_| unreachable!()),
            branch: "kiln/impl-demo".to_string(),
            title: "Implement".to_string(),
            body: "Verified".to_string(),
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
                // Read a bounded JSON body when one is announced.
                let text = String::from_utf8_lossy(&request).into_owned();
                if let Some(length) = text
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                {
                    if let Ok(length) = length.trim().parse::<usize>() {
                        let have = request.len().saturating_sub(
                            request
                                .windows(4)
                                .position(|window| window == b"\r\n\r\n")
                                .map_or(0, |position| position + 4),
                        );
                        let mut body = vec![0; length.saturating_sub(have)];
                        if !body.is_empty() {
                            stream
                                .read_exact(&mut body)
                                .unwrap_or_else(|_| unreachable!());
                            request.extend_from_slice(&body);
                        }
                    }
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

    fn branch_ref() -> String {
        serde_json::json!({"object": {"sha": "a".repeat(40)}}).to_string()
    }

    fn pull(number: u64, head: &str) -> serde_json::Value {
        serde_json::json!({
            "number": number,
            "state": "open",
            "merged": false,
            "html_url": format!("https://github.com/octocat/hello-world/pull/{number}"),
            "head": {"ref": "kiln/impl-demo", "sha": head, "user": {"login": "octocat"}},
            "base": {"ref": "main"}
        })
    }

    #[test]
    // r[verify kiln.github.publication] r[verify kiln.github.publication.scenario.create]
    fn creates_a_missing_branch_and_pull_request_idempotently() {
        let (endpoint, server) = serve(vec![
            ("404 Not Found", "{}".to_string()),
            ("201 Created", branch_ref()),
            ("200 OK", "[]".to_string()),
            ("201 Created", pull(9, &"a".repeat(40)).to_string()),
        ]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
        let branch = client
            .ensure_branch(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(branch.native_id.as_str(), "a".repeat(40));
        let outcome = client
            .ensure_pull_request(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(outcome.number, 9);
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(requests.first().is_some_and(|request| {
            request.starts_with("GET /repos/octocat/hello-world/git/ref/heads/kiln%2Fimpl-demo")
        }));
        assert!(requests.get(1).is_some_and(|request| {
            request.starts_with("POST /repos/octocat/hello-world/git/refs")
                && request.contains("refs/heads/kiln/impl-demo")
        }));
        assert!(requests.get(3).is_some_and(|request| {
            request.starts_with("POST /repos/octocat/hello-world/pulls")
                && request.contains("\"head\":\"kiln/impl-demo\"")
        }));
    }

    #[test]
    // r[verify kiln.github.publication] r[verify kiln.github.publication.scenario.existing]
    fn reuses_an_exact_existing_branch_and_pull_request() {
        let (endpoint, server) = serve(vec![
            ("200 OK", branch_ref()),
            (
                "200 OK",
                serde_json::json!([pull(9, &"a".repeat(40))]).to_string(),
            ),
        ]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
        client
            .ensure_branch(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let outcome = client
            .ensure_pull_request(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(outcome.number, 9);
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(requests.iter().all(|request| request.starts_with("GET ")));
    }

    #[test]
    // r[verify kiln.github.publication.scenario.existing]
    fn reuses_a_merged_pull_request_and_refuses_a_closed_unmerged_one() {
        let merged = serde_json::json!([{
            "number": 9,
            "state": "closed",
            "merged": true,
            "html_url": "https://github.com/octocat/hello-world/pull/9",
            "head": {"ref": "kiln/impl-demo", "sha": "a".repeat(40)},
            "base": {"ref": "main"}
        }])
        .to_string();
        let (endpoint, server) = serve(vec![("200 OK", merged)]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let outcome = client
            .ensure_pull_request(&plan())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(outcome.merged);
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(requests.len(), 1);

        let closed = serde_json::json!([{
            "number": 9,
            "state": "closed",
            "merged": false,
            "html_url": "https://github.com/octocat/hello-world/pull/9",
            "head": {"ref": "kiln/impl-demo", "sha": "a".repeat(40)},
            "base": {"ref": "main"}
        }])
        .to_string();
        let (endpoint, server) = serve(vec![("200 OK", closed)]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.ensure_pull_request(&plan()).is_err());
        let _ = server.join();
    }

    #[test]
    // r[verify kiln.github.publication] r[verify kiln.github.publication.scenario.refused]
    fn refuses_conflicting_branch_and_ambiguous_pull_request() {
        let mut malformed = plan();
        malformed.repository = "owner/repo/extra".to_string();
        let (endpoint, server) = serve(vec![]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.ensure_branch(&malformed).is_err());
        let _ = server.join();
        let mut malformed = plan();
        malformed.branch = "".to_string();
        let (endpoint, server) = serve(vec![]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.ensure_branch(&malformed).is_err());
        let _ = server.join();
        let conflict = serde_json::json!({"object": {"sha": "b".repeat(40)}}).to_string();
        let (endpoint, server) = serve(vec![("200 OK", conflict)]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.ensure_branch(&plan()).is_err());
        let _ = server.join();

        let (endpoint, server) = serve(vec![
            ("200 OK", branch_ref()),
            (
                "200 OK",
                serde_json::json!([pull(9, &"a".repeat(40)), pull(10, &"a".repeat(40))])
                    .to_string(),
            ),
        ]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
        client
            .ensure_branch(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(client.ensure_pull_request(&plan).is_err());
        let _ = server.join();
    }
}
