//! Read-only GitHub pull-request observation for guarded landing.
//!
//! This module inspects the exact PR, head, base and check runs. It never merges,
//! approves, updates or otherwise mutates remote state. Admission of an
//! observation is evidence for a separate landing authority, not permission.

use std::io::Read;
use std::time::Duration;

use kiln_core::types::{ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
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
    pub base_head: String,
    pub head_sha: String,
    pub required_checks: Vec<String>,
    pub required_approvals: u32,
    pub operation: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullObservation {
    pub number: u64,
    pub author: String,
    pub state: String,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub head_sha: String,
    pub base_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewObservation {
    pub actor: String,
    pub state: String,
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
    pub reviews: Vec<ReviewObservation>,
    pub base_head: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandingReceipt {
    pub pull_request: ExternalRef,
    pub candidate_head: String,
    pub merge_commit: String,
    pub base_ref: String,
    pub base_head: String,
    pub mapping: kiln_core::implementation::IntegrationMapping,
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
    admit_landing_evidence(plan, observation, false)
}

fn admit_landing_evidence(
    plan: &LandingPlan,
    observation: &LandingObservation,
    integrated: bool,
) -> Result<(), Diagnostic> {
    admit_plan(plan)?;
    let pull = &observation.pull;
    if pull.number != plan.pull_number
        || (!integrated && (pull.state != "open" || pull.merged || pull.mergeable != Some(true)))
        || (integrated && (pull.state != "closed" || !pull.merged))
        || pull.head_sha != plan.head_sha
        || pull.base_ref != plan.base_ref
        || (!integrated && observation.base_head != plan.base_head)
        || !commit(&observation.base_head)
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
    let mut latest = std::collections::BTreeMap::new();
    for review in &observation.reviews {
        if !bounded_name(&review.actor) || !bounded_name(&review.state) {
            return Err(refusal(
                "github_landing_review",
                "landing.reviews",
                "landing review actor or state is malformed",
            ));
        }
        latest.insert(review.actor.as_str(), review.state.as_str());
    }
    let mut approvals = 0_u32;
    for (actor, state) in latest {
        if state == "CHANGES_REQUESTED" {
            return Err(refusal(
                "github_landing_changes_requested",
                "landing.reviews",
                "a reviewer requested changes",
            ));
        }
        if state == "APPROVED" && actor != observation.pull.author {
            approvals = approvals.saturating_add(1);
        }
    }
    if approvals < plan.required_approvals {
        return Err(refusal(
            "github_landing_approvals",
            "landing.reviews",
            "landing does not have the required independent approvals",
        ));
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
        || !commit(&plan.base_head)
        || plan.required_checks.is_empty()
        || plan.required_checks.len() > MAX_CHECKS_REQUIRED
        || plan.required_approvals > 16
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
        let observation = self.read_observation(plan)?;
        admit_landing(plan, &observation)?;
        Ok(observation)
    }

    fn read_observation(&self, plan: &LandingPlan) -> PortResult<LandingObservation> {
        admit_plan(plan)?;
        let pull: RawPull = self.get_json(&format!(
            "{}/repos/{}/{}/pulls/{}",
            self.endpoint, plan.owner, plan.repository, plan.pull_number
        ))?;
        let checks: RawChecks = self.get_json_without_next(&format!(
            "{}/repos/{}/{}/commits/{}/check-runs?per_page={MAX_CHECKS}",
            self.endpoint, plan.owner, plan.repository, plan.head_sha
        ))?;
        if checks.total_count != checks.check_runs.len() || checks.total_count > MAX_CHECKS {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_landing_checks_pagination".to_string(),
                message: "landing check evidence is incomplete across pages".to_string(),
            }));
        }
        let reviews: Vec<RawReview> = self.get_json_without_next(&format!(
            "{}/repos/{}/{}/pulls/{}/reviews?per_page={MAX_CHECKS}",
            self.endpoint, plan.owner, plan.repository, plan.pull_number
        ))?;
        if reviews.len() > MAX_CHECKS {
            return Err(domain(
                "github_landing_reviews_bound",
                "landing.reviews",
                "review pagination is incomplete or exceeds its bound",
            ));
        }
        if reviews
            .iter()
            .any(|review| review.commit_id.as_deref() != Some(plan.head_sha.as_str()))
        {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_landing_review_revision".into(),
                message: "review does not bind the exact candidate revision".into(),
            }));
        }
        let base: RawRef = self.get_json(&format!(
            "{}/repos/{}/{}/git/ref/heads/{}",
            self.endpoint,
            plan.owner,
            plan.repository,
            encode(&plan.base_ref)
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
                author: pull.user.login,
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
            reviews: reviews
                .into_iter()
                .map(|review| ReviewObservation {
                    actor: review.user.login,
                    state: review.state,
                })
                .collect(),
            base_head: base.object.sha,
        };
        Ok(observation)
    }

    /// No available GitHub route atomically compares the admitted target head.
    /// The REST PR head guard is not a base guard. Keep the PR ready for review;
    /// even a successful readiness observation cannot authorize this mutation.
    // r[impl kiln.publication.atomicity]
    pub fn land(&self, plan: &LandingPlan) -> PortResult<LandingReceipt> {
        admit_plan(plan)?;
        Err(domain(
            "github_landing_unqualified_atomicity",
            "landing.strategy",
            "unattended landing is unsupported: no qualified atomic target guard; retain PR-ready",
        ))
    }

    /// Recover an already landed result without issuing another merge.
    ///
    /// Returns `None` when the PR is open and unmerged. A merged result is
    /// accepted only at the exact candidate head/base with a matching base
    /// read-back.
    // r[impl kiln.github.landing.merge]
    pub fn recover_landed(&self, plan: &LandingPlan) -> PortResult<Option<LandingReceipt>> {
        admit_plan(plan)?;
        let (owner, repository) = landing_repository(plan)?;
        let pull: RawPull = self.get_json(&format!(
            "{}/repos/{owner}/{repository}/pulls/{}",
            self.endpoint, plan.pull_number
        ))?;
        if !pull.merged {
            if pull.state != "open" {
                return Err(domain(
                    "github_landing_merge_mismatch",
                    "landing.merge",
                    "pull request is closed unmerged",
                ));
            }
            return Ok(None);
        }
        let Some(merge_commit) = pull.merge_commit_sha.as_deref() else {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_landing_merge_unknown".to_string(),
                message: "merged pull request has no observable merge commit".to_string(),
            }));
        };
        if pull.head.sha != plan.head_sha || pull.base.r#ref != plan.base_ref {
            return Err(domain(
                "github_landing_merge_mismatch",
                "landing.merge",
                "observed merged pull request does not match the exact candidate/base",
            ));
        }
        if !commit(merge_commit) {
            return Err(domain(
                "github_landing_merge_identity",
                "landing.merge",
                "malformed merge commit identity",
            ));
        }
        // A merged flag and ancestry are insufficient. The supported external
        // mapping has exact frozen base/head parents and the reviewed tree.
        let merge: RawCommit = self.get_json(&format!(
            "{}/repos/{owner}/{repository}/git/commits/{merge_commit}",
            self.endpoint
        ))?;
        let candidate: RawCommit = self.get_json(&format!(
            "{}/repos/{owner}/{repository}/git/commits/{}",
            self.endpoint, plan.head_sha
        ))?;
        admit_merge_mapping(plan, merge_commit, &merge, &candidate)?;
        let observation = self.read_observation(plan)?;
        admit_landing_evidence(plan, &observation, true)?;
        let base_head = observation.base_head;
        if base_head != merge_commit {
            let compare: RawCompare = self.get_json(&format!(
                "{}/repos/{owner}/{repository}/compare/{merge_commit}...{}",
                self.endpoint, base_head
            ))?;
            if !matches!(compare.status.as_str(), "ahead" | "identical")
                || compare.merge_base_commit.sha != merge_commit
            {
                return Err(PortFailure::Unknown(InfrastructureFailure {
                    code: "github_landing_base_unknown".to_string(),
                    message: "landed base head does not descend from the merge commit".to_string(),
                }));
            }
        }
        Ok(Some(LandingReceipt {
            pull_request: ExternalRef::new(
                "github",
                "pull_request",
                &format!("{owner}/{repository}#{}", plan.pull_number),
            )?,
            candidate_head: plan.head_sha.clone(),
            merge_commit: merge_commit.to_string(),
            base_ref: plan.base_ref.clone(),
            base_head,
            mapping: kiln_core::implementation::IntegrationMapping {
                expected_base: ExternalRef::new("git", "commit", &plan.base_head)?,
                candidate_tree: ExternalRef::new("git", "tree", &candidate.tree.sha)?,
                landed_tree: ExternalRef::new("git", "tree", &merge.tree.sha)?,
            },
        }))
    }

    fn decode<T: serde::de::DeserializeOwned>(&self, response: ureq::Response) -> PortResult<T> {
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

    fn get_json_without_next<T: serde::de::DeserializeOwned>(&self, url: &str) -> PortResult<T> {
        let response = self.get(url)?;
        if has_next_link(response.header("link"))? {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_landing_pagination".to_string(),
                message: "landing evidence is incomplete across pages".to_string(),
            }));
        }
        self.decode(response)
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
    user: RawUser,
    state: String,
    merged: bool,
    merge_commit_sha: Option<String>,
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
struct RawRef {
    object: RawRefObject,
}
#[derive(Deserialize)]
struct RawRefObject {
    sha: String,
}
#[derive(Deserialize)]
struct RawCommit {
    sha: String,
    tree: RawRefObject,
    parents: Vec<RawRefObject>,
}

fn admit_merge_mapping(
    plan: &LandingPlan,
    merge_sha: &str,
    merge: &RawCommit,
    candidate: &RawCommit,
) -> PortResult<()> {
    if merge.sha != merge_sha
        || candidate.sha != plan.head_sha
        || merge.parents.len() != 2
        || merge
            .parents
            .first()
            .is_none_or(|parent| parent.sha != plan.base_head)
        || merge
            .parents
            .get(1)
            .is_none_or(|parent| parent.sha != plan.head_sha)
        || !commit(&candidate.tree.sha)
        || merge.tree.sha != candidate.tree.sha
    {
        return Err(domain(
            "github_landing_merge_mapping",
            "landing.merge",
            "external merge does not preserve exact admitted base, head and reviewed tree",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawCompare {
    status: String,
    merge_base_commit: RawRefObject,
}
#[derive(Deserialize)]
struct RawChecks {
    total_count: usize,
    check_runs: Vec<RawCheck>,
}
#[derive(Deserialize)]
struct RawReview {
    user: RawUser,
    state: String,
    #[serde(default)]
    commit_id: Option<String>,
}
#[derive(Deserialize)]
struct RawUser {
    login: String,
}
#[derive(Deserialize)]
struct RawCheck {
    name: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
}

fn landing_repository(plan: &LandingPlan) -> PortResult<(&str, &str)> {
    let valid = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if !valid(&plan.owner) || !valid(&plan.repository) {
        return Err(domain(
            "github_landing_repository",
            "landing.repository",
            "landing repository identifiers are malformed",
        ));
    }
    Ok((&plan.owner, &plan.repository))
}

fn has_next_link(link: Option<&str>) -> PortResult<bool> {
    let Some(link) = link else {
        return Ok(false);
    };
    if link.len() > 2048
        || link.chars().any(char::is_control)
        || !link.split(',').all(|part| {
            let part = part.trim();
            part.starts_with('<')
                && part.contains(">; rel=\"")
                && [
                    "rel=\"next\"",
                    "rel=\"prev\"",
                    "rel=\"first\"",
                    "rel=\"last\"",
                ]
                .iter()
                .any(|relation| part.ends_with(*relation))
        })
    {
        return Err(domain(
            "github_landing_link",
            "landing.link",
            "GitHub landing Link header is malformed or oversized",
        ));
    }
    Ok(link.contains("rel=\"next\""))
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
            base_head: "b".repeat(40),
            head_sha: "a".repeat(40),
            required_checks: vec!["build".to_string(), "test".to_string()],
            required_approvals: 1,
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
            "user": {"login": "author"},
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
    fn detects_incomplete_landing_pagination() {
        assert!(
            has_next_link(Some("<https://api.github.com/x?page=2>; rel=\"next\""))
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(
            !has_next_link(Some("<https://api.github.com/x?page=1>; rel=\"prev\""))
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(has_next_link(Some(&"x".repeat(2049))).is_err());
        assert!(has_next_link(Some("malformed page metadata")).is_err());
    }

    #[test]
    // r[verify kiln.github.landing] r[verify kiln.github.landing.scenario.ready] r[verify kiln.github.landing.base] r[verify kiln.github.landing.base.scenario.exact]
    fn observes_an_exact_ready_pull_request() {
        let (endpoint, server) = serve(vec![
            ("200 OK", pull_body()),
            ("200 OK", checks_body()),
            (
                "200 OK",
                serde_json::json!([{"user": {"login": "reviewer"}, "state": "APPROVED", "commit_id": "a".repeat(40)}])
                    .to_string(),
            ),
            (
                "200 OK",
                serde_json::json!({"object": {"sha": "b".repeat(40)}}).to_string(),
            ),
        ]);
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
    // r[verify kiln.publication.atomicity.scenario.unsupported]
    fn unattended_landing_refuses_before_transport_even_with_ready_plan() {
        let (endpoint, server) = serve(vec![]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(client.land(&plan()), Err(PortFailure::Domain(_))));
        assert!(server.join().unwrap_or_else(|_| unreachable!()).is_empty());
    }

    fn merged_pull() -> String {
        serde_json::json!({"number": 7, "user": {"login": "author"}, "state": "closed",
            "merged": true, "merge_commit_sha": "c".repeat(40), "mergeable": false,
            "head": {"sha": "a".repeat(40)}, "base": {"ref": "main"}})
        .to_string()
    }

    fn commit_body(sha: char, tree: char, parents: &[char]) -> String {
        serde_json::json!({"sha": sha.to_string().repeat(40),
            "tree": {"sha": tree.to_string().repeat(40)},
            "parents": parents.iter().map(|parent| serde_json::json!({"sha": parent.to_string().repeat(40)})).collect::<Vec<_>>()}).to_string()
    }

    #[test]
    // r[verify kiln.publication.exact.scenario.external] r[verify kiln.github.landing.merge_ancestry] r[verify kiln.github.landing.merge_ancestry.scenario.advanced]
    fn reconciles_external_exact_merge_and_advanced_target_without_mutation() {
        for advanced in [false, true] {
            let mut responses = vec![
                ("200 OK", merged_pull()),
                ("200 OK", commit_body('c', 'e', &['b', 'a'])),
                ("200 OK", commit_body('a', 'e', &['b'])),
                ("200 OK", merged_pull()),
                ("200 OK", checks_body()),
                ("200 OK", serde_json::json!([{"user": {"login": "reviewer"}, "state": "APPROVED", "commit_id": "a".repeat(40)}]).to_string()),
                ("200 OK", serde_json::json!({"object": {"sha": if advanced {"d"} else {"c"}.repeat(40)}}).to_string()),
            ];
            if advanced {
                responses.push(("200 OK", serde_json::json!({"status": "ahead", "merge_base_commit": {"sha": "c".repeat(40)}}).to_string()));
            }
            let (endpoint, server) = serve(responses);
            let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
                .unwrap_or_else(|_| unreachable!());
            let receipt = client
                .recover_landed(&plan())
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .unwrap_or_else(|| unreachable!());
            assert_eq!(receipt.merge_commit, "c".repeat(40));
            assert_eq!(receipt.mapping.candidate_tree, receipt.mapping.landed_tree);
            let requests = server.join().unwrap_or_else(|_| unreachable!());
            assert!(requests.iter().all(|request| request.starts_with("GET ")));
        }
    }

    #[test]
    // r[verify kiln.github.landing]
    fn refuses_incomplete_check_counts() {
        let incomplete = serde_json::json!({
            "total_count": 3,
            "check_runs": [
                {"name": "build", "head_sha": "a".repeat(40), "status": "completed", "conclusion": "success"},
                {"name": "test", "head_sha": "a".repeat(40), "status": "completed", "conclusion": "success"}
            ]
        })
        .to_string();
        let (endpoint, _server) = serve(vec![("200 OK", pull_body()), ("200 OK", incomplete)]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.observe(&plan()).is_err());
    }

    #[test]
    // r[verify kiln.publication.exact.scenario.external] r[verify kiln.github.landing.merge.scenario.refused]
    fn refuses_base_race_squash_changed_tree_and_unknown_merge_mapping() {
        for (parents, tree) in [
            (vec!['d', 'a'], 'e'),
            (vec!['b'], 'e'),
            (vec!['b', 'a'], 'f'),
        ] {
            let (endpoint, server) = serve(vec![
                ("200 OK", merged_pull()),
                ("200 OK", commit_body('c', tree, &parents)),
                ("200 OK", commit_body('a', 'e', &['b'])),
            ]);
            let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
                .unwrap_or_else(|_| unreachable!());
            assert!(client.recover_landed(&plan()).is_err());
            let requests = server.join().unwrap_or_else(|_| unreachable!());
            assert!(requests.iter().all(|request| request.starts_with("GET ")));
        }
        let mut unknown: serde_json::Value =
            serde_json::from_str(&merged_pull()).unwrap_or_else(|_| unreachable!());
        *unknown
            .get_mut("merge_commit_sha")
            .unwrap_or_else(|| unreachable!()) = serde_json::Value::Null;
        let (endpoint, server) = serve(vec![("200 OK", unknown.to_string())]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.recover_landed(&plan()),
            Err(PortFailure::Unknown(_))
        ));
        let _ = server.join();
    }

    fn check(name: &str, head: &str, status: &str, conclusion: Option<&str>) -> CheckObservation {
        CheckObservation {
            name: name.to_string(),
            head_sha: head.to_string(),
            status: status.to_string(),
            conclusion: conclusion.map(str::to_string),
        }
    }

    fn review(actor: &str, state: &str) -> ReviewObservation {
        ReviewObservation {
            actor: actor.to_string(),
            state: state.to_string(),
        }
    }

    /// An observation the plan admits; each control changes exactly one fact.
    fn ready(plan: &LandingPlan) -> LandingObservation {
        LandingObservation {
            pull: PullObservation {
                number: plan.pull_number,
                author: "author".to_string(),
                state: "open".to_string(),
                merged: false,
                mergeable: Some(true),
                head_sha: plan.head_sha.clone(),
                base_ref: plan.base_ref.clone(),
            },
            checks: plan
                .required_checks
                .iter()
                .map(|name| check(name, &plan.head_sha, "completed", Some("success")))
                .collect(),
            reviews: vec![review("reviewer", "APPROVED")],
            base_head: plan.base_head.clone(),
        }
    }

    fn refusal_code(plan: &LandingPlan, observation: &LandingObservation) -> Option<String> {
        admit_landing(plan, observation)
            .err()
            .map(|diagnostic| diagnostic.code)
    }

    #[test]
    // r[verify kiln.github.landing] r[verify kiln.github.landing.scenario.refused] r[verify kiln.github.landing.base.scenario.moved] r[verify kiln.github.landing.approvals] r[verify kiln.github.landing.approvals.scenario.distinct]
    fn each_stale_or_incomplete_fact_alone_refuses_an_otherwise_ready_observation() {
        let plan = plan();
        assert_eq!(refusal_code(&plan, &ready(&plan)), None);
        type Change = fn(&mut LandingObservation);
        let controls: [(&str, Change, &str); 23] = [
            (
                "changed head",
                |o| o.pull.head_sha = "c".repeat(40),
                "github_landing_pull",
            ),
            (
                "moved base",
                |o| o.base_head = "c".repeat(40),
                "github_landing_pull",
            ),
            (
                "other base ref",
                |o| o.pull.base_ref = "release".to_string(),
                "github_landing_pull",
            ),
            ("other pull", |o| o.pull.number = 8, "github_landing_pull"),
            (
                "closed",
                |o| o.pull.state = "closed".to_string(),
                "github_landing_pull",
            ),
            ("merged", |o| o.pull.merged = true, "github_landing_pull"),
            (
                "unknown mergeability",
                |o| o.pull.mergeable = None,
                "github_landing_pull",
            ),
            (
                "conflicting",
                |o| o.pull.mergeable = Some(false),
                "github_landing_pull",
            ),
            (
                "missing check",
                |o| o.checks.retain(|c| c.name != "test"),
                "github_landing_check_missing",
            ),
            (
                "running check",
                |o| {
                    o.checks = vec![
                        check("build", &"a".repeat(40), "completed", Some("success")),
                        check("test", &"a".repeat(40), "in_progress", None),
                    ];
                },
                "github_landing_check_failed",
            ),
            (
                "failed check",
                |o| {
                    o.checks = vec![
                        check("build", &"a".repeat(40), "completed", Some("success")),
                        check("test", &"a".repeat(40), "completed", Some("failure")),
                    ];
                },
                "github_landing_check_failed",
            ),
            (
                "skipped check",
                |o| {
                    o.checks = vec![
                        check("build", &"a".repeat(40), "completed", Some("success")),
                        check("test", &"a".repeat(40), "completed", Some("skipped")),
                    ];
                },
                "github_landing_check_failed",
            ),
            (
                "neutral check",
                |o| {
                    o.checks = vec![
                        check("build", &"a".repeat(40), "completed", Some("success")),
                        check("test", &"a".repeat(40), "completed", Some("neutral")),
                    ];
                },
                "github_landing_check_failed",
            ),
            (
                "check on another head",
                |o| {
                    o.checks
                        .push(check("lint", &"c".repeat(40), "completed", Some("success")))
                },
                "github_landing_check",
            ),
            (
                "duplicate check",
                |o| {
                    o.checks.push(check(
                        "build",
                        &"a".repeat(40),
                        "completed",
                        Some("success"),
                    ))
                },
                "github_landing_check",
            ),
            (
                "unnamed check",
                |o| {
                    o.checks
                        .push(check("", &"a".repeat(40), "completed", Some("success")))
                },
                "github_landing_check",
            ),
            (
                "no review",
                |o| o.reviews.clear(),
                "github_landing_approvals",
            ),
            (
                "author approval only",
                |o| o.reviews = vec![review("author", "APPROVED")],
                "github_landing_approvals",
            ),
            (
                "approval dismissed",
                |o| o.reviews.push(review("reviewer", "DISMISSED")),
                "github_landing_approvals",
            ),
            (
                "approval then changes requested",
                |o| o.reviews.push(review("reviewer", "CHANGES_REQUESTED")),
                "github_landing_changes_requested",
            ),
            (
                "another reviewer requests changes",
                |o| o.reviews.push(review("other", "CHANGES_REQUESTED")),
                "github_landing_changes_requested",
            ),
            (
                "unnamed reviewer",
                |o| o.reviews.push(review("", "APPROVED")),
                "github_landing_review",
            ),
            (
                "control-character state",
                |o| o.reviews.push(review("other", "APPROVED\n")),
                "github_landing_review",
            ),
        ];
        for (name, change, code) in controls {
            let mut observation = ready(&plan);
            change(&mut observation);
            assert_eq!(
                refusal_code(&plan, &observation).as_deref(),
                Some(code),
                "{name}"
            );
        }
        // The latest review per actor counts, and only required checks gate.
        let admitted: [(&str, Change); 3] = [
            ("changes requested then approved", |o| {
                o.reviews = vec![
                    review("reviewer", "CHANGES_REQUESTED"),
                    review("reviewer", "APPROVED"),
                ];
            }),
            ("commented beside an approval", |o| {
                o.reviews.push(review("other", "COMMENTED"))
            }),
            ("failed optional check", |o| {
                o.checks
                    .push(check("docs", &"a".repeat(40), "completed", Some("failure")))
            }),
        ];
        for (name, change) in admitted {
            let mut observation = ready(&plan);
            change(&mut observation);
            assert_eq!(refusal_code(&plan, &observation), None, "{name}");
        }
    }

    #[test]
    // r[verify kiln.github.landing.scenario.refused] r[verify kiln.github.landing.approvals.scenario.distinct]
    fn a_stricter_current_policy_refuses_evidence_admitted_under_the_old_one() {
        let old = plan();
        let observation = ready(&old);
        assert_eq!(refusal_code(&old, &observation), None);
        let mut more_approvals = old.clone();
        more_approvals.required_approvals = 2;
        assert_eq!(
            refusal_code(&more_approvals, &observation).as_deref(),
            Some("github_landing_approvals")
        );
        let mut more_checks = old.clone();
        more_checks.required_checks.push("lint".to_string());
        assert_eq!(
            refusal_code(&more_checks, &observation).as_deref(),
            Some("github_landing_check_missing")
        );
        let mut moved = old.clone();
        moved.base_head = "d".repeat(40);
        assert_eq!(
            refusal_code(&moved, &observation).as_deref(),
            Some("github_landing_pull")
        );
        for malformed in [
            LandingPlan {
                required_checks: Vec::new(),
                ..old.clone()
            },
            LandingPlan {
                required_checks: vec!["build".to_string(), "build".to_string()],
                ..old.clone()
            },
            LandingPlan {
                schema: "kiln.github-landing-plan.v0".to_string(),
                ..old.clone()
            },
        ] {
            assert_eq!(
                refusal_code(&malformed, &observation).as_deref(),
                Some("github_landing_plan")
            );
        }
    }

    #[test]
    // r[verify kiln.github.landing.scenario.refused] r[verify kiln.github.landing.approvals]
    fn an_approval_on_an_earlier_head_is_never_reused() {
        let (endpoint, server) = serve(vec![
            ("200 OK", pull_body()),
            ("200 OK", checks_body()),
            (
                "200 OK",
                serde_json::json!([{"user": {"login": "reviewer"}, "state": "APPROVED", "commit_id": "c".repeat(40)}])
                    .to_string(),
            ),
        ]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.observe(&plan()),
            Err(PortFailure::Unknown(InfrastructureFailure { code, .. })) if code == "github_landing_review_revision"
        ));
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(requests.iter().all(|request| request.starts_with("GET ")));
    }

    #[test]
    // r[verify kiln.github.landing.merge.scenario.refused]
    fn an_unmerged_pull_request_recovers_nothing_and_a_closed_one_refuses() {
        let (endpoint, server) = serve(vec![("200 OK", pull_body())]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(client.recover_landed(&plan()), Ok(None)));
        assert_eq!(server.join().unwrap_or_else(|_| unreachable!()).len(), 1);
        let mut closed: serde_json::Value =
            serde_json::from_str(&pull_body()).unwrap_or_else(|_| unreachable!());
        *closed.get_mut("state").unwrap_or_else(|| unreachable!()) = "closed".into();
        let (endpoint, server) = serve(vec![("200 OK", closed.to_string())]);
        let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.recover_landed(&plan()),
            Err(PortFailure::Domain(diagnostic)) if diagnostic.code == "github_landing_merge_mismatch"
        ));
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert!(requests.iter().all(|request| request.starts_with("GET ")));
    }
}
