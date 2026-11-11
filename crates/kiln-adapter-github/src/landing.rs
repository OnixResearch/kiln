//! Read-only GitHub pull-request observation for guarded landing.
//!
//! This module inspects the exact PR, head, base and check runs. It never merges,
//! approves, updates or otherwise mutates remote state. Admission of an
//! observation is evidence for a separate landing authority, not permission.

use std::io::Read;
use std::time::Duration;

use kiln_core::types::{ContentId, Diagnostic};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHECKS: usize = 100;
const MAX_CHECKS_REQUIRED: usize = 32;
const MAX_NAME_BYTES: usize = 256;
pub const LANDING_PLAN_SCHEMA: &str = "kiln.github-landing-plan.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandingPlan {
    pub schema: String,
    pub owner: String,
    pub repository: String,
    pub pull_number: u64,
    pub base_ref: String,
    pub head_sha: String,
    pub required_checks: Vec<String>,
    pub operation: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullObservation {
    pub number: u64,
    pub state: String,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub head_sha: String,
    pub base_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckObservation {
    pub name: String,
    pub head_sha: String,
    pub status: String,
    pub conclusion: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandingObservation {
    pub pull: PullObservation,
    pub checks: Vec<CheckObservation>,
}

/// Admit a read-only observation as exact landing evidence.
///
/// It requires the PR to be open, unmerged, mergeable, at the exact head/base,
/// with every required check completed successfully on that head. It does not
/// merge or authorize landing.
// r[impl kiln.github.landing]
pub fn admit_landing(
    plan: &LandingPlan,
    observation: &LandingObservation,
) -> Result<(), Diagnostic> {
    admit_plan(plan)?;
    let pull = &observation.pull;
    if pull.number != plan.pull_number
        || pull.state != "open"
        || pull.merged
        || pull.mergeable != Some(true)
        || pull.head_sha != plan.head_sha
        || pull.base_ref != plan.base_ref
    {
        return Err(refusal(
            "github_landing_pull",
            "landing.pull",
            "pull request is not open, unmerged, mergeable at the exact head and base",
        ));
    }
    let mut checks = std::collections::BTreeMap::new();
    for check in &observation.checks {
        if check.head_sha != plan.head_sha
            || !bounded_name(&check.name)
            || checks.insert(check.name.as_str(), check).is_some()
        {
            return Err(refusal(
                "github_landing_check",
                "landing.checks",
                "landing check identity is malformed, stale or duplicated",
            ));
        }
    }
    for required in &plan.required_checks {
        let Some(check) = checks.get(required.as_str()) else {
            return Err(refusal(
                "github_landing_check_missing",
                "landing.checks",
                "required landing check is absent",
            ));
        };
        if check.status != "completed" || check.conclusion.as_deref() != Some("success") {
            return Err(refusal(
                "github_landing_check_failed",
                "landing.checks",
                "required landing check did not complete successfully",
            ));
        }
    }
    Ok(())
}

fn admit_plan(plan: &LandingPlan) -> Result<(), Diagnostic> {
    if plan.schema != LANDING_PLAN_SCHEMA
        || !identifier(&plan.owner)
        || !identifier(&plan.repository)
        || plan.pull_number == 0
        || !branch(&plan.base_ref)
        || !commit(&plan.head_sha)
        || plan.required_checks.is_empty()
        || plan.required_checks.len() > MAX_CHECKS_REQUIRED
        || plan.operation.validate().is_err()
    {
        return Err(refusal(
            "github_landing_plan",
            "landing.plan",
            "landing plan is malformed or over bound",
        ));
    }
    let mut names = std::collections::BTreeSet::new();
    for name in &plan.required_checks {
        if !bounded_name(name) || !names.insert(name.as_str()) {
            return Err(refusal(
                "github_landing_plan",
                "landing.plan.required_checks",
                "required landing checks are malformed or duplicated",
            ));
        }
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn branch(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("..")
        && !value.chars().any(char::is_control)
}

fn bounded_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_NAME_BYTES && !value.chars().any(char::is_control)
}

fn commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Read-only GitHub landing observer.
pub struct GithubLandingClient {
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubLandingClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubLandingClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl GithubLandingClient {
    pub fn new(token: ResolvedSecret, timeout_ms: u64) -> PortResult<Self> {
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(test)]
    fn new_fixture(token: ResolvedSecret, timeout_ms: u64, endpoint: &str) -> PortResult<Self> {
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_landing_endpoint",
                "landing.endpoint",
                "fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_landing_endpoint",
                "landing.endpoint",
                "fixture endpoint must declare a port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_landing_endpoint",
                "landing.endpoint",
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
                "github_landing_timeout",
                "landing.timeout_ms",
                "landing timeout must be positive",
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
                "github_landing_unauthorized",
                "landing.http",
                "GitHub landing observation credentials were refused",
            )),
            Err(ureq::Error::Status(404, _)) => Err(domain(
                "github_landing_not_found",
                "landing.http",
                "GitHub pull request or commit was not found",
            )),
            Err(ureq::Error::Status(status, _)) => Err(infrastructure(
                "github_landing_status",
                &format!("GitHub landing observation returned HTTP {status}"),
            )),
            Err(_) => Err(infrastructure(
                "github_landing_transport",
                "GitHub landing observation transport failed",
            )),
        }
    }

    /// Read on-demand evidence. No remote mutation occurs.
    // r[impl kiln.github.landing]
    pub fn observe(&self, plan: &LandingPlan) -> PortResult<LandingObservation> {
        admit_plan(plan)?;
        let pull: RawPull = self.get_json(&format!(
            "{}/repos/{}/{}/pulls/{}",
            self.endpoint, plan.owner, plan.repository, plan.pull_number
        ))?;
        let checks: RawChecks = self.get_json(&format!(
            "{}/repos/{}/{}/commits/{}/check-runs?per_page={MAX_CHECKS}",
            self.endpoint, plan.owner, plan.repository, plan.head_sha
        ))?;
        if checks.total_count > MAX_CHECKS || checks.check_runs.len() > MAX_CHECKS {
            return Err(domain(
                "github_landing_checks_bound",
                "landing.checks",
                "check-run pagination is incomplete or exceeds its bound",
            ));
        }
        let observation = LandingObservation {
            pull: PullObservation {
                number: pull.number,
                state: pull.state,
                merged: pull.merged,
                mergeable: pull.mergeable,
                head_sha: pull.head.sha,
                base_ref: pull.base.r#ref,
            },
            checks: checks
                .check_runs
                .into_iter()
                .map(|check| CheckObservation {
                    name: check.name,
                    head_sha: check.head_sha,
                    status: check.status,
                    conclusion: check.conclusion,
                })
                .collect(),
        };
        admit_landing(plan, &observation)?;
        Ok(observation)
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> PortResult<T> {
        let response = self.get(url)?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| {
                infrastructure(
                    "github_landing_body",
                    "GitHub landing observation body could not be read",
                )
            })?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(domain(
                "github_landing_bound",
                "landing.response",
                "GitHub landing observation exceeds its byte bound",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_landing_payload",
                "landing.response",
                "GitHub landing observation payload is malformed",
            )
        })
    }
}

#[derive(Deserialize)]
struct RawPull {
    number: u64,
    state: String,
    merged: bool,
    mergeable: Option<bool>,
    head: RawHead,
    base: RawBase,
}
#[derive(Deserialize)]
struct RawHead {
    sha: String,
}
#[derive(Deserialize)]
struct RawBase {
    #[serde(rename = "ref")]
    r#ref: String,
}
#[derive(Deserialize)]
struct RawChecks {
    total_count: usize,
    check_runs: Vec<RawCheck>,
}
#[derive(Deserialize)]
struct RawCheck {
    name: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn token() -> ResolvedSecret {
        ResolvedSecret::from_bytes(b"fixture-token".to_vec()).unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> LandingPlan {
        LandingPlan {
            schema: LANDING_PLAN_SCHEMA.to_string(),
            owner: "octocat".to_string(),
            repository: "hello-world".to_string(),
            pull_number: 7,
            base_ref: "main".to_string(),
            head_sha: "a".repeat(40),
            required_checks: vec!["build".to_string(), "test".to_string()],
            operation: ContentId::from_blake3_hex("b".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
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

    fn pull_body() -> String {
        serde_json::json!({
            "number": 7,
            "state": "open",
            "merged": false,
            "mergeable": true,
            "head": {"sha": "a".repeat(40)},
            "base": {"ref": "main"}
        })
        .to_string()
    }

    fn checks_body() -> String {
        serde_json::json!({
            "total_count": 2,
            "check_runs": [
                {"name": "build", "head_sha": "a".repeat(40), "status": "completed", "conclusion": "success"},
                {"name": "test", "head_sha": "a".repeat(40), "status": "completed", "conclusion": "success"}
            ]
        })
        .to_string()
    }

    #[test]
    // r[verify kiln.github.landing] r[verify kiln.github.landing.scenario.ready]
    fn observes_an_exact_ready_pull_request() {
        let (endpoint, server) = serve(vec![("200 OK", pull_body()), ("200 OK", checks_body())]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let observation = client
            .observe(&plan())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(observation.checks.len(), 2);
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(
            requests.first().is_some_and(
                |request| request.starts_with("GET /repos/octocat/hello-world/pulls/7")
            )
        );
        assert!(requests.get(1).is_some_and(|request| {
            request.contains("/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs")
                && request.contains("per_page=100")
        }));
    }

    #[test]
    // r[verify kiln.github.landing] r[verify kiln.github.landing.scenario.refused]
    fn refuses_stale_or_incomplete_observations() {
        let plan = plan();
        let mut observation = LandingObservation {
            pull: PullObservation {
                number: 7,
                state: "open".to_string(),
                merged: false,
                mergeable: Some(true),
                head_sha: plan.head_sha.clone(),
                base_ref: "main".to_string(),
            },
            checks: vec![CheckObservation {
                name: "build".to_string(),
                head_sha: plan.head_sha.clone(),
                status: "completed".to_string(),
                conclusion: Some("success".to_string()),
            }],
        };
        assert!(admit_landing(&plan, &observation).is_err());
        observation.pull.head_sha = "c".repeat(40);
        assert!(admit_landing(&plan, &observation).is_err());
        observation.pull.head_sha = plan.head_sha.clone();
        observation.checks.push(CheckObservation {
            name: "build".to_string(),
            head_sha: plan.head_sha.clone(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
        });
        assert!(admit_landing(&plan, &observation).is_err());
    }
}
