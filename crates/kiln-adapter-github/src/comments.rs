//! Bounded GitHub REST client for PR fork origin verification, issue/comment
//! polling, and idempotent issue/PR comment status replies (`#99`).

use std::io::Read;
use std::time::Duration;

use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::implementation::timestamp_millis;
use crate::poll_http::next_page;
use crate::runbook_routing::{PullRequestHead, PullRequestHeadRepo, PullRequestOriginCheck};
use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PAGES: u32 = 20;
const MAX_ACTOR_BYTES: usize = 128;
const MAX_COMMENT_BODY_BYTES: usize = 262_144;
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueCommentPollRecord {
    pub comment_id: u64,
    pub issue_number: u64,
    pub body: String,
    pub actor: String,
    pub actor_type: Option<String>,
    pub author_association: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub is_pull_request: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueCommentPollPage {
    pub records: Vec<IssueCommentPollRecord>,
    pub next_page: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRoutingPollRecord {
    pub number: u64,
    pub state: String,
    pub updated_at_ms: u64,
    pub actor: String,
    pub actor_type: Option<String>,
    pub author_association: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRoutingPollPage {
    pub records: Vec<IssueRoutingPollRecord>,
    pub next_page: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostedIssueComment {
    pub comment_id: u64,
}

pub trait GithubCommentPort: PullRequestOriginCheck {
    fn list_issue_comments(
        &mut self,
        owner: &str,
        repository: &str,
        since_ms: u64,
        page: u32,
        per_page: u8,
    ) -> PortResult<IssueCommentPollPage>;

    fn list_issues_for_routing(
        &mut self,
        owner: &str,
        repository: &str,
        since_ms: u64,
        page: u32,
        per_page: u8,
    ) -> PortResult<IssueRoutingPollPage>;

    fn post_issue_comment(
        &mut self,
        owner: &str,
        repository: &str,
        issue_number: u64,
        body: &str,
    ) -> PortResult<PostedIssueComment>;

    fn find_issue_comment_by_marker(
        &mut self,
        owner: &str,
        repository: &str,
        issue_number: u64,
        marker: &str,
    ) -> PortResult<Option<PostedIssueComment>>;
}

pub struct GithubCommentClient {
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubCommentClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubCommentClient")
            .field("endpoint", &self.endpoint)
            .field("token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl GithubCommentClient {
    pub fn new(token: ResolvedSecret, timeout_ms: u64) -> PortResult<Self> {
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(any(test, feature = "protocol-fixtures"))]
    pub fn new_fixture(token: ResolvedSecret, timeout_ms: u64, endpoint: &str) -> PortResult<Self> {
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_comment_endpoint",
                "comments.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_comment_endpoint",
                "comments.endpoint",
                "fixture endpoint must declare a loopback port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_comment_endpoint",
                "comments.endpoint",
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
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(domain(
                "github_comment_timeout",
                "comments.timeout_ms",
                "GitHub comment client timeout must be positive and bounded",
            ));
        }
        if !token.as_bytes().iter().all(|b| (b'!'..=b'~').contains(b)) {
            return Err(domain(
                "github_comment_token",
                "secret",
                "GitHub token is not valid header material",
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

    fn request(&self, method: &str, path: &str) -> PortResult<ureq::Request> {
        let authorization = Zeroizing::new(format!("Bearer {}", self.token.as_str()?));
        Ok(self
            .agent
            .request(method, &format!("{}{path}", self.endpoint))
            .set("Authorization", &authorization)
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("User-Agent", "kiln-github-adapter"))
    }

    fn read_response(&self, response: ureq::Response, mutation: bool) -> PortResult<Vec<u8>> {
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                if mutation {
                    unknown(
                        "github_comment_body_unknown",
                        "GitHub comment response was incomplete; reconciliation required",
                    )
                } else {
                    infrastructure(
                        "github_comment_body",
                        "GitHub response body could not be read",
                    )
                }
            })?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(domain(
                "github_comment_response_bound",
                "comments.response",
                "GitHub response exceeded its byte bound",
            ));
        }
        Ok(bytes)
    }

    fn check_status(
        &self,
        result: Result<ureq::Response, ureq::Error>,
        mutation: bool,
        expected: u16,
    ) -> PortResult<ureq::Response> {
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => {
                return Err(if mutation {
                    unknown(
                        "github_comment_transport_unknown",
                        "GitHub comment write outcome is unknown; reconciliation required",
                    )
                } else {
                    infrastructure("github_comment_transport", "GitHub API transport failed")
                });
            }
        };
        let status = response.status();
        if status == expected {
            return Ok(response);
        }
        if status == 429
            || (status == 403
                && (response.header("X-RateLimit-Remaining") == Some("0")
                    || response.header("Retry-After").is_some()))
        {
            return Err(PortFailure::Retryable(InfrastructureFailure {
                code: "github_comment_rate_limited".to_string(),
                message: "GitHub API request was rate limited".to_string(),
            }));
        }
        if matches!(status, 401 | 403 | 404) {
            return Err(domain(
                "github_comment_denied",
                "comments.http",
                &format!("GitHub API refused request with HTTP {status}"),
            ));
        }
        if mutation && (status >= 500 || status == 408 || (200..300).contains(&status)) {
            return Err(unknown(
                "github_comment_response_unknown",
                "GitHub comment write may have succeeded; reconciliation required",
            ));
        }
        Err(infrastructure(
            "github_comment_http_status",
            &format!("GitHub API returned HTTP {status}"),
        ))
    }

    pub fn get_pull_request_head(
        &mut self,
        owner: &str,
        repository: &str,
        number: u64,
    ) -> PortResult<PullRequestHead> {
        if !identifier(owner) || !identifier(repository) || number == 0 {
            return Err(domain(
                "github_pr_lookup_request",
                "pull_request",
                "pull request lookup parameters are invalid",
            ));
        }
        let path = format!("/repos/{owner}/{repository}/pulls/{number}");
        let response = self.check_status(self.request("GET", &path)?.call(), false, 200)?;
        let bytes = self.read_response(response, false)?;
        let raw: RawPullRequest = serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_pr_lookup_payload",
                "pull_request",
                "GitHub pull request lookup response is malformed",
            )
        })?;
        Ok(PullRequestHead {
            repo: raw.head.repo.map(|repo| PullRequestHeadRepo {
                id: repo.id,
                name: repo.name,
                full_name: repo.full_name,
                owner_login: repo.owner.login,
                fork: repo.fork,
            }),
        })
    }
}

impl PullRequestOriginCheck for GithubCommentClient {
    fn check_pull_request_head(
        &mut self,
        owner: &str,
        repository: &str,
        pull_request_number: u64,
    ) -> PortResult<PullRequestHead> {
        self.get_pull_request_head(owner, repository, pull_request_number)
    }
}

impl GithubCommentPort for GithubCommentClient {
    fn list_issue_comments(
        &mut self,
        owner: &str,
        repository: &str,
        since_ms: u64,
        page: u32,
        per_page: u8,
    ) -> PortResult<IssueCommentPollPage> {
        if !identifier(owner)
            || !identifier(repository)
            || page == 0
            || per_page == 0
            || per_page > 100
        {
            return Err(domain(
                "github_comment_poll_request",
                "comments.poll",
                "GitHub comment poll parameters are invalid",
            ));
        }
        let mut path = format!(
            "/repos/{owner}/{repository}/issues/comments?sort=created&direction=asc&per_page={per_page}&page={page}"
        );
        if since_ms != 0 {
            path.push_str("&since=");
            path.push_str(&rfc3339(since_ms)?);
        }
        let response = self.check_status(self.request("GET", &path)?.call(), false, 200)?;
        let next = next_page(response.header("link"), page)?;
        let bytes = self.read_response(response, false)?;
        let raw: Vec<RawIssueComment> = serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_comment_poll_payload",
                "comments.poll",
                "GitHub comment poll response is not a comment array",
            )
        })?;
        if raw.len() > usize::from(per_page) {
            return Err(domain(
                "github_comment_poll_bound",
                "comments.poll",
                "GitHub comment poll page exceeds requested size",
            ));
        }
        let mut records = Vec::with_capacity(raw.len());
        for item in raw {
            if item.id == 0
                || item.body.len() > MAX_COMMENT_BODY_BYTES
                || item.user.login.is_empty()
                || item.user.login.len() > MAX_ACTOR_BYTES
                || item.user.login.chars().any(char::is_control)
            {
                return Err(domain(
                    "github_comment_poll_record",
                    "comments.poll",
                    "GitHub comment poll record is malformed",
                ));
            }
            let issue_number = parse_issue_number_from_url(&item.issue_url, owner, repository)?;
            let created_at_ms = timestamp_millis(&item.created_at).ok_or_else(|| {
                domain(
                    "github_comment_poll_timestamp",
                    "comments.poll.created_at",
                    "GitHub comment record has an invalid created_at timestamp",
                )
            })?;
            let updated_at_ms = match item.updated_at.as_deref() {
                Some(ts) => timestamp_millis(ts).ok_or_else(|| {
                    domain(
                        "github_comment_poll_timestamp",
                        "comments.poll.updated_at",
                        "GitHub comment record has an invalid updated_at timestamp",
                    )
                })?,
                None => created_at_ms,
            };
            let is_pull_request = item.pull_request.is_some()
                || !item
                    .html_url
                    .as_deref()
                    .is_some_and(|url| html_url_is_issue(url, owner, repository, issue_number));
            records.push(IssueCommentPollRecord {
                comment_id: item.id,
                issue_number,
                body: item.body,
                actor: item.user.login,
                actor_type: item.user.actor_type,
                author_association: item.author_association,
                created_at_ms,
                updated_at_ms,
                is_pull_request,
            });
        }
        Ok(IssueCommentPollPage {
            records,
            next_page: next,
        })
    }

    fn list_issues_for_routing(
        &mut self,
        owner: &str,
        repository: &str,
        since_ms: u64,
        page: u32,
        per_page: u8,
    ) -> PortResult<IssueRoutingPollPage> {
        if !identifier(owner)
            || !identifier(repository)
            || page == 0
            || per_page == 0
            || per_page > 100
        {
            return Err(domain(
                "github_issue_poll_request",
                "issues.poll",
                "GitHub issue poll parameters are invalid",
            ));
        }
        let mut path = format!(
            "/repos/{owner}/{repository}/issues?state=open&sort=updated&direction=asc&per_page={per_page}&page={page}"
        );
        if since_ms != 0 {
            path.push_str("&since=");
            path.push_str(&rfc3339(since_ms)?);
        }
        let response = self.check_status(self.request("GET", &path)?.call(), false, 200)?;
        let next = next_page(response.header("link"), page)?;
        let bytes = self.read_response(response, false)?;
        let raw: Vec<RawRoutingIssue> = serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_issue_poll_payload",
                "issues.poll",
                "GitHub issue poll response is malformed",
            )
        })?;
        if raw.len() > usize::from(per_page) {
            return Err(domain(
                "github_issue_poll_bound",
                "issues.poll",
                "GitHub issue poll page exceeds requested size",
            ));
        }
        let mut records = Vec::with_capacity(raw.len());
        for item in raw {
            if item.pull_request.is_some() {
                continue;
            }
            if item.number == 0
                || item.user.login.is_empty()
                || item.user.login.len() > MAX_ACTOR_BYTES
                || item.user.login.chars().any(char::is_control)
                || !matches!(item.state.as_str(), "open" | "closed")
            {
                return Err(domain(
                    "github_issue_poll_record",
                    "issues.poll",
                    "GitHub issue poll record is malformed",
                ));
            }
            let updated_at_ms = timestamp_millis(&item.updated_at).ok_or_else(|| {
                domain(
                    "github_issue_poll_timestamp",
                    "issues.poll.updated_at",
                    "GitHub issue poll record has an invalid timestamp",
                )
            })?;
            records.push(IssueRoutingPollRecord {
                number: item.number,
                state: item.state,
                updated_at_ms,
                actor: item.user.login,
                actor_type: item.user.actor_type,
                author_association: item.author_association,
                labels: item.labels.into_iter().map(|label| label.name).collect(),
            });
        }
        Ok(IssueRoutingPollPage {
            records,
            next_page: next,
        })
    }

    fn post_issue_comment(
        &mut self,
        owner: &str,
        repository: &str,
        issue_number: u64,
        body: &str,
    ) -> PortResult<PostedIssueComment> {
        if !identifier(owner)
            || !identifier(repository)
            || issue_number == 0
            || body.trim().is_empty()
            || body.len() > 65_536
        {
            return Err(domain(
                "github_comment_post_request",
                "comments.post",
                "GitHub issue comment post request is invalid",
            ));
        }
        let payload = serde_json::to_vec(&CreateCommentRequest { body }).map_err(|_| {
            infrastructure(
                "github_comment_encode",
                "GitHub issue comment payload could not be encoded",
            )
        })?;
        let path = format!("/repos/{owner}/{repository}/issues/{issue_number}/comments");
        let response = self.check_status(
            self.request("POST", &path)?
                .set("Content-Type", "application/json")
                .send_bytes(&payload),
            true,
            201,
        )?;
        let bytes = self.read_response(response, true)?;
        let created: RawCreatedComment = serde_json::from_slice(&bytes).map_err(|_| {
            unknown(
                "github_comment_malformed_unknown",
                "GitHub comment create response was malformed; reconciliation required",
            )
        })?;
        if created.id == 0 {
            return Err(unknown(
                "github_comment_malformed_unknown",
                "GitHub comment create response omitted comment id; reconciliation required",
            ));
        }
        Ok(PostedIssueComment {
            comment_id: created.id,
        })
    }

    fn find_issue_comment_by_marker(
        &mut self,
        owner: &str,
        repository: &str,
        issue_number: u64,
        marker: &str,
    ) -> PortResult<Option<PostedIssueComment>> {
        if !identifier(owner)
            || !identifier(repository)
            || issue_number == 0
            || marker.is_empty()
            || marker.len() > 256
        {
            return Err(domain(
                "github_comment_lookup_request",
                "comments.reconcile",
                "GitHub comment reconciliation request is invalid",
            ));
        }
        let mut page = 1_u32;
        for _ in 0..MAX_PAGES {
            let path = format!(
                "/repos/{owner}/{repository}/issues/{issue_number}/comments?per_page=100&page={page}"
            );
            let response = self.check_status(self.request("GET", &path)?.call(), false, 200)?;
            let next = next_page(response.header("link"), page)?;
            let bytes = self.read_response(response, false)?;
            let items: Vec<RawListedComment> = serde_json::from_slice(&bytes).map_err(|_| {
                unknown(
                    "github_comment_lookup_malformed",
                    "GitHub comment reconciliation response was malformed",
                )
            })?;
            for item in &items {
                if item.id != 0 && item.body.contains(marker) {
                    return Ok(Some(PostedIssueComment {
                        comment_id: item.id,
                    }));
                }
            }
            match next {
                Some(next_num) if next_num > page => page = next_num,
                _ => return Ok(None),
            }
        }
        Err(unknown(
            "github_comment_pages_exhausted",
            "GitHub comment reconciliation exhausted its page bound",
        ))
    }
}

#[derive(Serialize)]
struct CreateCommentRequest<'a> {
    body: &'a str,
}

#[derive(Deserialize)]
struct RawCreatedComment {
    id: u64,
}

#[derive(Deserialize)]
struct RawListedComment {
    id: u64,
    body: String,
}

#[derive(Deserialize)]
struct RawPullRequest {
    head: RawPullRequestHead,
}

#[derive(Deserialize)]
struct RawPullRequestHead {
    #[serde(default)]
    repo: Option<RawPullRequestRepo>,
}

#[derive(Deserialize)]
struct RawPullRequestRepo {
    id: u64,
    name: String,
    #[serde(default)]
    full_name: Option<String>,
    owner: RawUser,
    #[serde(default)]
    fork: bool,
}

#[derive(Deserialize)]
struct RawIssueComment {
    id: u64,
    body: String,
    user: RawUser,
    #[serde(default)]
    author_association: Option<String>,
    created_at: String,
    #[serde(default)]
    updated_at: Option<String>,
    issue_url: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawRoutingIssue {
    number: u64,
    state: String,
    updated_at: String,
    user: RawUser,
    #[serde(default)]
    author_association: Option<String>,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
    #[serde(default)]
    labels: Vec<RawLabel>,
}

#[derive(Deserialize)]
struct RawLabel {
    name: String,
}

#[derive(Deserialize)]
struct RawUser {
    login: String,
    #[serde(default, rename = "type")]
    actor_type: Option<String>,
}

fn html_url_is_issue(url: &str, owner: &str, repository: &str, issue_number: u64) -> bool {
    let Some(after_scheme) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let Some((host, path_and_rest)) = after_scheme.split_once('/') else {
        return false;
    };
    if host.is_empty() {
        return false;
    }
    let path = path_and_rest.split(['#', '?']).next().unwrap_or_default();
    let mut parts = path.split('/');
    let (Some(url_owner), Some(url_repo), Some("issues"), Some(num_str), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return false;
    };
    url_owner.eq_ignore_ascii_case(owner)
        && url_repo.eq_ignore_ascii_case(repository)
        && num_str.parse::<u64>().ok() == Some(issue_number)
}

fn parse_issue_number_from_url(url: &str, owner: &str, repository: &str) -> PortResult<u64> {
    let Some(repos_idx) = url.find("/repos/") else {
        return Err(domain(
            "github_comment_poll_record",
            "comments.issue_url",
            "GitHub comment issue_url is malformed",
        ));
    };
    let tail = url.get(repos_idx + "/repos/".len()..).ok_or_else(|| {
        domain(
            "github_comment_poll_record",
            "comments.issue_url",
            "GitHub comment issue_url is malformed",
        )
    })?;
    let mut parts = tail.split('/');
    let (Some(url_owner), Some(url_repo), Some("issues"), Some(num_str), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Err(domain(
            "github_comment_poll_record",
            "comments.issue_url",
            "GitHub comment issue_url does not match expected path",
        ));
    };
    if !url_owner.eq_ignore_ascii_case(owner) || !url_repo.eq_ignore_ascii_case(repository) {
        return Err(domain(
            "github_comment_poll_record",
            "comments.issue_url",
            "GitHub comment issue_url repository mismatches policy",
        ));
    }
    num_str
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            domain(
                "github_comment_poll_record",
                "comments.issue_url",
                "GitHub comment issue_url issue number is invalid",
            )
        })
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn unknown(code: &str, message: &str) -> PortFailure {
    PortFailure::Unknown(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

fn rfc3339(ms: u64) -> PortResult<String> {
    let seconds = i64::try_from(ms / 1_000).map_err(|_| {
        domain(
            "github_comment_timestamp",
            "poll.since",
            "poll timestamp exceeds its supported range",
        )
    })?;
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
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
            "github_comment_timestamp",
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

    #[test]
    fn list_issue_comments_fails_closed_unless_html_url_is_issue() {
        let payload = r#"[
            {
                "id": 101,
                "body": "/runbook flake-update",
                "user": {"login": "maintainer", "type": "User"},
                "author_association": "OWNER",
                "created_at": "2026-11-11T10:00:00Z",
                "updated_at": "2026-11-11T10:00:00Z",
                "issue_url": "https://api.github.com/repos/OnixResearch/kiln/issues/8",
                "html_url": "https://github.com/OnixResearch/kiln/issues/8#issuecomment-101"
            },
            {
                "id": 102,
                "body": "/runbook flake-update",
                "user": {"login": "maintainer", "type": "User"},
                "author_association": "OWNER",
                "created_at": "2026-11-11T10:01:00Z",
                "updated_at": "2026-11-11T10:01:00Z",
                "issue_url": "https://api.github.com/repos/OnixResearch/kiln/issues/9"
            },
            {
                "id": 103,
                "body": "/runbook flake-update",
                "user": {"login": "maintainer", "type": "User"},
                "author_association": "OWNER",
                "created_at": "2026-11-11T10:02:00Z",
                "updated_at": "2026-11-11T10:02:00Z",
                "issue_url": "https://api.github.com/repos/OnixResearch/kiln/issues/10",
                "html_url": "https://github.com/OnixResearch/kiln/pull/10#issuecomment-103"
            },
            {
                "id": 104,
                "body": "/runbook flake-update",
                "user": {"login": "maintainer", "type": "User"},
                "author_association": "OWNER",
                "created_at": "2026-11-11T10:03:00Z",
                "updated_at": "2026-11-11T10:03:00Z",
                "issue_url": "https://api.github.com/repos/OnixResearch/kiln/issues/11",
                "html_url": "https://github.com/OnixResearch/kiln/issues/11#issuecomment-104",
                "pull_request": {}
            }
        ]"#;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
            let _ = crate::tests::read_http_request(&mut stream);
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            stream
                .write_all(reply.as_bytes())
                .unwrap_or_else(|_| unreachable!());
        });
        let token =
            ResolvedSecret::from_bytes(b"gh-secret".to_vec()).unwrap_or_else(|_| unreachable!());
        let mut client = GithubCommentClient::new_fixture(token, 5_000, &format!("http://{addr}"))
            .unwrap_or_else(|_| unreachable!());
        let page = client
            .list_issue_comments("OnixResearch", "kiln", 0, 1, 50)
            .unwrap_or_else(|_| unreachable!());
        handle.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(page.records.len(), 4);
        assert_eq!(
            page.records
                .iter()
                .map(|record| record.is_pull_request)
                .collect::<Vec<_>>(),
            vec![false, true, true, true]
        );
    }
}
