//! Idempotent verified-PR publication to GitHub.
//!
//! The caller supplies an admitted publication plan and operator-owned source
//! profile; candidate objects are pushed through guarded native Git before PR
//! reconciliation. No merge or source-status mutation is performed here.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git2::{Cred, CredentialType, Oid, Repository};
use kiln_adapter_git::native_publication::{
    NativeRemoteProfile, NativeTransport, PushObservation, push_exact_ref, read_remote_ref,
    remote_identity,
};
use kiln_core::implementation::{
    MAX_STACK_LAYERS, MIN_STACK_LAYERS, PublicationPlan, STACK_PUBLICATION_SCHEMA, STACK_SCHEMA,
    StackPlan, StackPublication, admit_stack_publication, layer_publication_plans, stack_identity,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::Deserialize;

use crate::secret::ResolvedSecret;
use crate::status::BoundedGithubResolver;
use crate::{domain, infrastructure};
/// Operator-owned source and remote authority for candidate publication.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationSourceProfile {
    pub schema: String,
    pub repository_path: PathBuf,
    pub remote: NativeRemoteProfile,
    pub remote_identity: ContentId,
    pub fixture: bool,
}

pub const PUBLICATION_SOURCE_PROFILE_SCHEMA: &str = "kiln.github-publication-source.v1";

impl PublicationSourceProfile {
    pub fn read(path: &Path) -> PortResult<Self> {
        let bytes = std::fs::read(path).map_err(|_| {
            domain(
                "github_publication_source_profile",
                "publication.source_profile",
                "source profile is unreadable",
            )
        })?;
        serde_json::from_slice(&bytes).map_err(|_| {
            domain(
                "github_publication_source_profile",
                "publication.source_profile",
                "source profile is malformed",
            )
        })
    }
}

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
        // Default builds never read this qualification-only transport override.
        #[cfg(feature = "protocol-fixtures")]
        if let Some(endpoint) = std::env::var_os("KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT") {
            let endpoint = endpoint.into_string().map_err(|_| {
                domain(
                    "github_publication_endpoint",
                    "publication.endpoint",
                    "fixture endpoint must be a UTF-8 loopback HTTP origin",
                )
            })?;
            return Self::new_fixture(token, timeout_ms, &endpoint);
        }
        Self::build(token, timeout_ms, API.to_string(), true)
    }

    #[cfg(any(test, feature = "protocol-fixtures"))]
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

    /// Upload and reconcile the exact captured candidate through native Git.
    /// The caller persists its effect intent before invoking this operation.
    pub fn publish_candidate_branch(
        &self,
        plan: &PublicationPlan,
        source: &PublicationSourceProfile,
        push: bool,
    ) -> PortResult<Option<ExternalRef>> {
        validate_plan(plan)?;
        validate_source_profile(plan, source, &self.endpoint)?;
        let path = candidate_repository_path(plan, source)?;
        let repository = Repository::open_bare(&path)
            .or_else(|_| Repository::open(&path))
            .map_err(|_| publication_source_error())?;
        let oid = Oid::from_str(plan.candidate.native_id.as_str())
            .map_err(|_| publication_source_error())?;
        let commit = repository
            .find_commit(oid)
            .map_err(|_| publication_source_error())?;
        if commit.tree_id().to_string() != plan.tree.native_id.as_str() {
            return Err(domain(
                "github_publication_tree_conflict",
                "publication.tree",
                "captured candidate tree differs from verified plan",
            ));
        }
        let reference = format!("refs/heads/{}", plan.branch);
        let mut credential = |url: &str, _username: Option<&str>, allowed: CredentialType| {
            if source.fixture
                || url != source.remote.remote_url
                || !allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
            {
                return Err(git2::Error::from_str(
                    "credential request is outside enrolled GitHub HTTPS remote",
                ));
            }
            let token = self
                .token
                .as_str()
                .map_err(|_| git2::Error::from_str("GitHub token is unavailable"))?;
            Cred::userpass_plaintext("x-access-token", token)
        };
        let current = read_remote_ref(
            &repository,
            &source.remote,
            &source.remote_identity,
            &reference,
            &mut credential,
        )
        .map_err(|_| publication_remote_error())?;
        match current {
            Some(found) if found == oid => return Ok(Some(commit_ref(&found.to_string())?)),
            Some(_) => {
                return Err(domain(
                    "github_publication_branch_conflict",
                    "publication.branch",
                    "existing branch head differs from candidate; overwrite is forbidden",
                ));
            }
            None if !push => return Ok(None),
            None => {}
        }
        match push_exact_ref(
            &repository,
            &source.remote,
            &source.remote_identity,
            oid,
            &reference,
            Oid::ZERO_SHA1,
            &mut credential,
        )
        .map_err(|_| publication_remote_error())?
        {
            PushObservation::Confirmed { commit } if commit == oid.to_string() => {
                Ok(Some(commit_ref(&commit)?))
            }
            PushObservation::Rejected { .. }
            | PushObservation::Unknown { .. }
            | PushObservation::Confirmed { .. } => {
                let reconciled = read_remote_ref(
                    &repository,
                    &source.remote,
                    &source.remote_identity,
                    &reference,
                    &mut credential,
                )
                .map_err(|_| publication_remote_error())?;
                match reconciled {
                    Some(found) if found == oid => Ok(Some(commit_ref(&found.to_string())?)),
                    Some(_) => Err(domain(
                        "github_publication_branch_conflict",
                        "publication.branch",
                        "remote branch has an unexpected head; overwrite is forbidden",
                    )),
                    None => Err(PortFailure::Unknown(InfrastructureFailure {
                        code: "github_publication_branch_unknown".into(),
                        message:
                            "candidate push outcome remains unresolved; reconcile before retry"
                                .into(),
                    })),
                }
            }
        }
    }

    /// Ensure exactly one open PR exists for the candidate branch and base.
    // r[impl kiln.github.publication]
    pub fn ensure_pull_request(&self, plan: &PublicationPlan) -> PortResult<PublicationOutcome> {
        if let Some(outcome) = self.find_pull_request(plan, true)? {
            return Ok(outcome);
        }
        self.create_pull_request(plan, true)
    }

    /// Inspect all PR states so closed or externally merged PRs cannot be
    /// mistaken for absence after a crash. Any incomplete page is unknown.
    pub fn reconcile_pull_request(
        &self,
        plan: &PublicationPlan,
    ) -> PortResult<Option<PublicationOutcome>> {
        self.find_pull_request(plan, true)
    }

    /// With `by_base` false the head branch alone selects the pull request,
    /// so a stack layer that GitHub retargeted after a lower merge is found
    /// and refused by `outcome` instead of being created a second time.
    fn find_pull_request(
        &self,
        plan: &PublicationPlan,
        by_base: bool,
    ) -> PortResult<Option<PublicationOutcome>> {
        validate_plan(plan)?;
        let (owner, repository) = repository_parts(&plan.repository)?;
        let base = if by_base {
            format!("&base={}", encode(&plan.base_branch))
        } else {
            String::new()
        };
        let list = format!(
            "{}/repos/{owner}/{repository}/pulls?state=all&head={}:{}{base}&per_page=100",
            self.endpoint,
            encode(owner),
            encode(&plan.branch),
        );
        let response = self.get(&list).map_err(HttpError::into_failure)?;
        if has_next_link(response.header("link"))? {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "github_publication_pr_pagination".to_string(),
                message: "pull request reconciliation is incomplete across pages".to_string(),
            }));
        }
        let pulls: Vec<RawPull> = self.decode(response)?;
        if pulls.len() > 100 || pulls.len() > 1 {
            return Err(unknown(
                "github_publication_ambiguous",
                "more than one matching pull request or incomplete result exists",
            ));
        }
        match pulls.first() {
            Some(pull) => Ok(Some(outcome(plan, pull)?)),
            None => Ok(None),
        }
    }

    fn create_pull_request(
        &self,
        plan: &PublicationPlan,
        by_base: bool,
    ) -> PortResult<PublicationOutcome> {
        validate_plan(plan)?;
        let (owner, repository) = repository_parts(&plan.repository)?;
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
                let pull: RawPull = self.decode(response).map_err(|_| {
                    unknown(
                        "github_publication_pr_unknown",
                        "PR mutation response is unreadable; reconcile before retry",
                    )
                })?;
                outcome(plan, &pull)
            }
            Err(HttpError::Status(422 | 409)) => {
                self.find_pull_request(plan, by_base)?.ok_or_else(|| {
                    unknown(
                        "github_publication_pr_race",
                        "pull request creation raced and remained unreconciled",
                    )
                })
            }
            Err(error) => Err(error.into_mutation_failure()),
        }
    }

    /// Whether the repository has stacked pull requests. GitHub answers 404
    /// from every stack endpoint when they are not enabled.
    // r[impl kiln.github.stack]
    pub fn supports_stacks(&self, repository: &str) -> PortResult<bool> {
        let (owner, name) = repository_parts(repository)?;
        match self.get(&format!(
            "{}/repos/{owner}/{name}/stacks?per_page=1",
            self.endpoint
        )) {
            Ok(response) => self.decode::<Vec<RawStack>>(response).map(|_| true),
            Err(HttpError::Status(404)) => Ok(false),
            Err(error) => Err(error.into_failure()),
        }
    }

    /// Publish a stack plan as exact layer branches, pull requests and one
    /// GitHub stack, bottom-up. Support, every layer's tree and ancestry, and
    /// every layer's remote state are read before the first mutation, so a
    /// re-run reuses what exists and mutates nothing. Kiln never adds to,
    /// removes from, retargets or dissolves a stack.
    // r[impl kiln.github.stack]
    pub fn publish_stack(
        &self,
        plan: &StackPlan,
        source: &PublicationSourceProfile,
    ) -> PortResult<StackPublication> {
        let layers = validate_stack_plan(plan)?;
        if !self.supports_stacks(&plan.repository)? {
            return Err(domain(
                "github_stack_unsupported",
                "stack.repository",
                "repository has no stacked pull requests; publish one pull request instead",
            ));
        }
        check_stack_layers(&layers, source, &self.endpoint)?;
        let mut existing = Vec::with_capacity(layers.len());
        for layer in &layers {
            let pull = self.find_pull_request(layer, false)?;
            if pull.is_none() {
                self.publish_candidate_branch(layer, source, false)?;
            }
            existing.push(pull);
        }
        let mut pulls = Vec::with_capacity(layers.len());
        for (layer, pull) in layers.iter().zip(existing) {
            let pull = match pull {
                Some(pull) => pull,
                None => {
                    self.publish_candidate_branch(layer, source, true)?
                        .ok_or_else(publication_remote_error)?;
                    self.create_pull_request(layer, false)?
                }
            };
            pulls.push(pull);
        }
        let number = self.ensure_stack(plan, &pulls)?;
        let publication = StackPublication {
            schema: STACK_PUBLICATION_SCHEMA.to_string(),
            stack: stack_identity(plan)?,
            provider_stack: ExternalRef::new(
                "github",
                "stack",
                &format!("{}#{number}", plan.repository),
            )?,
            pull_requests: pulls.into_iter().map(|pull| pull.pull_request).collect(),
        };
        admit_stack_publication(plan, &publication)?;
        Ok(publication)
    }

    /// Reuse the exact stack or create it once. A conflict, lost response or
    /// unreadable body is reconciled once and otherwise stays unknown.
    fn ensure_stack(&self, plan: &StackPlan, pulls: &[PublicationOutcome]) -> PortResult<u64> {
        if let Some(number) = self.find_stack(plan, pulls)? {
            return Ok(number);
        }
        let (owner, repository) = repository_parts(&plan.repository)?;
        let numbers: Vec<u64> = pulls.iter().map(|pull| pull.number).collect();
        let body = serde_json::json!({ "pull_requests": numbers }).to_string();
        let created = match self.post(
            &format!("{}/repos/{owner}/{repository}/stacks", self.endpoint),
            &body,
        ) {
            Ok(response) => self.decode::<RawStack>(response).ok(),
            Err(HttpError::Status(status @ (401 | 403 | 404))) => {
                return Err(HttpError::Status(status).into_failure());
            }
            Err(_) => None,
        };
        match created {
            Some(stack) => {
                exact_stack(plan, pulls, &stack)?;
                Ok(stack.number)
            }
            None => self.find_stack(plan, pulls)?.ok_or_else(|| {
                unknown(
                    "github_stack_unknown",
                    "stack creation outcome remains unresolved; reconcile before retry",
                )
            }),
        }
    }

    fn find_stack(
        &self,
        plan: &StackPlan,
        pulls: &[PublicationOutcome],
    ) -> PortResult<Option<u64>> {
        let (owner, repository) = repository_parts(&plan.repository)?;
        let bottom = pulls.first().ok_or_else(stack_plan_error)?.number;
        let response = self
            .get(&format!(
                "{}/repos/{owner}/{repository}/stacks?pull_request={bottom}&per_page=100",
                self.endpoint
            ))
            .map_err(HttpError::into_failure)?;
        if has_next_link(response.header("link"))? {
            return Err(unknown(
                "github_stack_pagination",
                "stack reconciliation is incomplete across pages",
            ));
        }
        let stacks: Vec<RawStack> = self.decode(response)?;
        match stacks.as_slice() {
            [] => Ok(None),
            [stack] => exact_stack(plan, pulls, stack).map(|()| Some(stack.number)),
            _ => Err(unknown(
                "github_stack_ambiguous",
                "more than one stack contains the bottom pull request",
            )),
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

fn validate_source_profile(
    plan: &PublicationPlan,
    source: &PublicationSourceProfile,
    endpoint: &str,
) -> PortResult<()> {
    if source.schema != PUBLICATION_SOURCE_PROFILE_SCHEMA
        || !source.repository_path.is_absolute()
        || remote_identity(&source.remote).ok().as_ref() != Some(&source.remote_identity)
        || (source.fixture != (source.remote.transport == NativeTransport::LocalBareFixture))
    {
        return Err(domain(
            "github_publication_source_profile",
            "publication.source_profile",
            "source profile identity or fixture transport is invalid",
        ));
    }
    let root = source
        .repository_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(publication_source_error)?;
    let expected = root.join(format!(
        "{}-{}/candidate",
        plan.attempt.as_str(),
        plan.fence
    ));
    let actual =
        std::fs::canonicalize(&source.repository_path).map_err(|_| publication_source_error())?;
    let expected = std::fs::canonicalize(expected).map_err(|_| publication_source_error())?;
    if actual != expected
        || !actual.starts_with(std::fs::canonicalize(root).map_err(|_| publication_source_error())?)
    {
        return Err(domain(
            "github_publication_source_locator",
            "publication.source",
            "profile repository does not resolve to the exact attempt/fence candidate export",
        ));
    }
    match source.remote.transport {
        NativeTransport::LocalBareFixture => {
            #[cfg(not(any(test, feature = "protocol-fixtures")))]
            return Err(domain(
                "github_publication_fixture_disabled",
                "publication.source_profile",
                "local bare publication is fixture-only",
            ));
            #[cfg(any(test, feature = "protocol-fixtures"))]
            if !endpoint.starts_with("http://127.0.0.1:") && !endpoint.starts_with("http://[::1]:")
            {
                return Err(domain(
                    "github_publication_fixture_endpoint",
                    "publication.source_profile",
                    "local bare Git publication requires loopback GitHub fixture context",
                ));
            }
        }
        NativeTransport::Https => {
            let (owner, repository) = repository_parts(&plan.repository)?;
            let expected_url = format!("https://github.com/{owner}/{repository}.git");
            if source.fixture || source.remote.remote_url != expected_url || endpoint != API {
                return Err(domain(
                    "github_publication_remote_mismatch",
                    "publication.remote",
                    "GitHub token may only publish to the enrolled HTTPS repository",
                ));
            }
        }
        NativeTransport::SshStrictHostKey => {
            return Err(domain(
                "github_publication_transport",
                "publication.remote",
                "GitHub publication requires HTTPS or explicit local fixture transport",
            ));
        }
    }
    Ok(())
}

fn candidate_repository_path(
    plan: &PublicationPlan,
    source: &PublicationSourceProfile,
) -> PortResult<PathBuf> {
    let root = source
        .repository_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(publication_source_error)?;
    let path = root.join(format!(
        "{}-{}/candidate",
        plan.attempt.as_str(),
        plan.fence
    ));
    std::fs::canonicalize(path).map_err(|_| publication_source_error())
}

/// Shape of a stack plan the adapter relies on; full admission is the core's.
fn validate_stack_plan(plan: &StackPlan) -> PortResult<Vec<PublicationPlan>> {
    repository_parts(&plan.repository)?;
    let layers = layer_publication_plans(plan);
    let chained = layers.iter().enumerate().all(|(index, layer)| {
        match index.checked_sub(1).and_then(|below| layers.get(below)) {
            None => layer.base_branch == plan.trunk_branch && layer.base == plan.base,
            Some(below) => layer.base_branch == below.branch && layer.base == below.candidate,
        }
    });
    if plan.schema != STACK_SCHEMA
        || !(MIN_STACK_LAYERS..=MAX_STACK_LAYERS).contains(&layers.len())
        || !chained
    {
        return Err(stack_plan_error());
    }
    for layer in &layers {
        validate_plan(layer)?;
    }
    Ok(layers)
}

/// Check every layer's tree and ancestry in the captured export before any push.
fn check_stack_layers(
    layers: &[PublicationPlan],
    source: &PublicationSourceProfile,
    endpoint: &str,
) -> PortResult<()> {
    let top = layers.last().ok_or_else(stack_plan_error)?;
    validate_source_profile(top, source, endpoint)?;
    let path = candidate_repository_path(top, source)?;
    let repository = Repository::open_bare(&path)
        .or_else(|_| Repository::open(&path))
        .map_err(|_| publication_source_error())?;
    for layer in layers {
        let oid = Oid::from_str(layer.candidate.native_id.as_str())
            .map_err(|_| publication_source_error())?;
        let parent =
            Oid::from_str(layer.base.native_id.as_str()).map_err(|_| publication_source_error())?;
        let commit = repository
            .find_commit(oid)
            .map_err(|_| publication_source_error())?;
        if commit.tree_id().to_string() != layer.tree.native_id.as_str() {
            return Err(domain(
                "github_publication_tree_conflict",
                "publication.tree",
                "captured candidate tree differs from verified plan",
            ));
        }
        if !repository
            .graph_descendant_of(oid, parent)
            .map_err(|_| publication_source_error())?
        {
            return Err(domain(
                "github_stack_ancestry",
                "stack.layers",
                "layer candidate does not descend from its planned parent",
            ));
        }
    }
    Ok(())
}

fn exact_stack(plan: &StackPlan, pulls: &[PublicationOutcome], stack: &RawStack) -> PortResult<()> {
    let exact = stack.number != 0
        && stack.base.r#ref == plan.trunk_branch
        && stack.pull_requests.len() == plan.layers.len()
        && pulls.len() == plan.layers.len()
        && stack
            .pull_requests
            .iter()
            .zip(plan.layers.iter().zip(pulls))
            .all(|(member, (layer, pull))| {
                member.number == pull.number
                    && member.head.r#ref == layer.branch
                    && member.head.sha == layer.candidate.native_id.as_str()
            });
    if exact {
        return Ok(());
    }
    Err(domain(
        "github_stack_conflict",
        "stack.provider",
        "existing stack differs from the plan; Kiln never edits or dissolves a stack",
    ))
}

fn stack_plan_error() -> PortFailure {
    domain(
        "github_stack_plan",
        "stack.plan",
        "stack plan has malformed layers or a broken parent or branch chain",
    )
}

fn publication_source_error() -> PortFailure {
    domain(
        "github_publication_source",
        "publication.source",
        "trusted captured candidate repository is unavailable or malformed",
    )
}

fn publication_remote_error() -> PortFailure {
    PortFailure::Unknown(InfrastructureFailure {
        code: "github_publication_remote_unknown".into(),
        message: "native Git publication state could not be reconciled".into(),
    })
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
        || plan.candidate.provider_namespace.as_str() != "git"
        || plan.candidate.native_kind.as_str() != "commit"
        || plan.tree.provider_namespace.as_str() != "git"
        || plan.tree.native_kind.as_str() != "tree"
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
    if pull.number == 0
        || pull.head.r#ref != plan.branch
        || pull.head.sha != plan.candidate.native_id.as_str()
        || pull.base.r#ref != plan.base_branch
        || !(pull.state == "open" || (pull.state == "closed" && pull.is_merged()))
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
        merged: pull.is_merged(),
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

    fn into_mutation_failure(self) -> PortFailure {
        match self {
            Self::Status(401 | 403 | 404) => self.into_failure(),
            _ => unknown(
                "github_publication_mutation_unknown",
                "mutation transport outcome is unknown; reconcile before any retry",
            ),
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

fn unknown(code: &str, message: &str) -> PortFailure {
    PortFailure::Unknown(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

#[derive(Deserialize)]
struct RawPull {
    number: u64,
    state: String,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    merged_at: Option<String>,
    html_url: String,
    head: RawHead,
    base: RawBase,
}

impl RawPull {
    fn is_merged(&self) -> bool {
        self.merged || self.merged_at.is_some()
    }
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

/// The stack resource; list, create and get share these fields.
#[derive(Deserialize)]
struct RawStack {
    number: u64,
    base: RawBase,
    pull_requests: Vec<RawStackPull>,
}
#[derive(Deserialize)]
struct RawStackPull {
    number: u64,
    head: RawHead,
}

#[cfg(test)]
#[path = "publication_stack_tests.rs"]
mod stack_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::types::ContentId;
    use std::io::Write as _;
    use std::net::TcpListener;

    #[test]
    fn detects_incomplete_publication_pagination() {
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

    pub(super) fn token() -> ResolvedSecret {
        ResolvedSecret::from_bytes(b"fixture-token".to_vec()).unwrap_or_else(|_| unreachable!())
    }

    fn captured_fixture() -> (
        tempfile::TempDir,
        PublicationPlan,
        PublicationSourceProfile,
        PathBuf,
    ) {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let remote_path = directory.path().join("remote.git");
        let _remote = Repository::init_bare(&remote_path).unwrap_or_else(|_| unreachable!());
        let export_root = directory.path().join("candidate-export");
        let capture = export_root.join(format!(
            "{}-{}/candidate",
            plan().attempt.as_str(),
            plan().fence
        ));
        std::fs::create_dir_all(&capture).unwrap_or_else(|_| unreachable!());
        let repository = Repository::init(&capture).unwrap_or_else(|_| unreachable!());
        let tree_oid = repository
            .treebuilder(None)
            .unwrap_or_else(|_| unreachable!())
            .write()
            .unwrap_or_else(|_| unreachable!());
        let tree = repository
            .find_tree(tree_oid)
            .unwrap_or_else(|_| unreachable!());
        let signature = git2::Signature::now("Kiln fixture", "fixture@example.invalid")
            .unwrap_or_else(|_| unreachable!());
        let candidate = repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "captured candidate",
                &tree,
                &[],
            )
            .unwrap_or_else(|_| unreachable!());
        let remote = remote_path.to_string_lossy().into_owned();
        repository
            .remote("origin", &remote)
            .unwrap_or_else(|_| unreachable!());
        drop(tree);
        drop(repository);
        std::fs::create_dir_all(&export_root).unwrap_or_else(|_| unreachable!());
        symlink(
            capture.parent().unwrap_or_else(|| unreachable!()),
            export_root.join("current"),
        )
        .unwrap_or_else(|_| unreachable!());
        let mut publication = plan();
        publication.candidate = ExternalRef::new("git", "commit", &candidate.to_string())
            .unwrap_or_else(|_| unreachable!());
        publication.tree = ExternalRef::new("git", "tree", &tree_oid.to_string())
            .unwrap_or_else(|_| unreachable!());
        let remote_profile = NativeRemoteProfile {
            remote_name: "origin".into(),
            remote_url: remote,
            transport: NativeTransport::LocalBareFixture,
            ssh_host_key_sha256: None,
        };
        let source = PublicationSourceProfile {
            schema: PUBLICATION_SOURCE_PROFILE_SCHEMA.into(),
            repository_path: export_root.join("current/candidate"),
            remote_identity: remote_identity(&remote_profile).unwrap_or_else(|_| unreachable!()),
            remote: remote_profile,
            fixture: true,
        };
        (directory, publication, source, remote_path)
    }

    fn fixture_endpoint() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        format!(
            "http://{}",
            listener.local_addr().unwrap_or_else(|_| unreachable!())
        )
    }

    #[test]
    // r[verify kiln.github.publication.scenario.upload]
    fn uploads_and_reads_back_the_exact_candidate_objects_before_pr_transport() {
        let (_directory, plan, source, remote_path) = captured_fixture();
        let client = GithubPublicationClient::new_fixture(token(), 5000, &fixture_endpoint())
            .unwrap_or_else(|_| unreachable!());
        let branch = client
            .publish_candidate_branch(&plan, &source, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .unwrap_or_else(|| unreachable!());
        assert_eq!(branch.native_id, plan.candidate.native_id);
        let remote = Repository::open_bare(remote_path).unwrap_or_else(|_| unreachable!());
        let remote_ref = remote
            .find_reference("refs/heads/kiln/impl-demo")
            .unwrap_or_else(|_| unreachable!());
        let commit = remote
            .find_commit(remote_ref.target().unwrap_or_else(|| unreachable!()))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(commit.id().to_string(), plan.candidate.native_id.as_str());
        assert_eq!(commit.tree_id().to_string(), plan.tree.native_id.as_str());
    }

    #[test]
    // r[verify kiln.github.publication.scenario.refused]
    fn refuses_wrong_tree_and_non_attempt_candidate_locator_before_push() {
        let (_directory, plan, source, remote_path) = captured_fixture();
        let client = GithubPublicationClient::new_fixture(token(), 5000, &fixture_endpoint())
            .unwrap_or_else(|_| unreachable!());
        let mut wrong_tree = plan.clone();
        wrong_tree.tree =
            ExternalRef::new("git", "tree", &"f".repeat(40)).unwrap_or_else(|_| unreachable!());
        assert!(
            client
                .publish_candidate_branch(&wrong_tree, &source, true)
                .is_err()
        );
        let mut wrong_locator = source;
        wrong_locator.repository_path = wrong_locator.repository_path.with_file_name("untrusted");
        assert!(
            client
                .publish_candidate_branch(&plan, &wrong_locator, true)
                .is_err()
        );
        let remote = Repository::open_bare(remote_path).unwrap_or_else(|_| unreachable!());
        assert!(remote.find_reference("refs/heads/kiln/impl-demo").is_err());
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

    pub(super) fn serve(
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
                    && let Ok(length) = length.trim().parse::<usize>()
                {
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
    fn creates_a_missing_pull_request_after_branch_publication() {
        let (endpoint, server) = serve(vec![
            ("200 OK", "[]".to_string()),
            ("201 Created", pull(9, &"a".repeat(40)).to_string()),
        ]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
        let outcome = client
            .ensure_pull_request(&plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(outcome.number, 9);
        assert_eq!(outcome.branch_head, plan.candidate);
        drop(server.join().unwrap_or_else(|_| unreachable!()));
    }

    #[test]
    // r[verify kiln.github.publication] r[verify kiln.github.publication.scenario.existing]
    fn reuses_an_exact_existing_pull_request() {
        let (endpoint, server) = serve(vec![(
            "200 OK",
            serde_json::json!([pull(9, &"a".repeat(40))]).to_string(),
        )]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
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
    fn refuses_malformed_plan_and_ambiguous_pull_request() {
        let mut malformed = plan();
        malformed.branch.clear();
        assert!(validate_plan(&malformed).is_err());
        malformed = plan();
        malformed.repository = "owner/repo/extra".to_string();
        assert!(repository_parts(&malformed.repository).is_err());

        let (endpoint, server) = serve(vec![(
            "200 OK",
            serde_json::json!([pull(9, &"a".repeat(40)), pull(10, &"a".repeat(40))]).to_string(),
        )]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(client.ensure_pull_request(&plan()).is_err());
        let _ = server.join();
    }

    #[test]
    // r[verify kiln.publication.recovery.scenario.crash]
    fn lost_mutation_ack_reconciles_branch_and_pr_without_another_post() {
        let (endpoint, server) = serve(vec![
            ("200 OK", "[]".into()),
            ("201 Created", "lost or truncated body".into()),
            (
                "200 OK",
                serde_json::json!([pull(9, &"a".repeat(40))]).to_string(),
            ),
        ]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan();
        assert!(client.ensure_pull_request(&plan).is_err());
        assert_eq!(
            client
                .reconcile_pull_request(&plan)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!())
                .number,
            9
        );
        let requests = server.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            1
        );
        assert!(
            requests
                .last()
                .is_some_and(|request| request.contains("state=all"))
        );
    }

    #[test]
    fn incomplete_page_and_open_plus_merged_matches_cannot_authorize_creation() {
        let (endpoint, server) = serve(vec![(
            "200 OK\r\nLink: <https://api.github.com/pulls?page=2>; rel=\"next\"",
            "[]".into(),
        )]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.ensure_pull_request(&plan()),
            Err(PortFailure::Unknown(_))
        ));
        assert!(
            server
                .join()
                .unwrap_or_else(|_| unreachable!())
                .iter()
                .all(|request| request.starts_with("GET "))
        );
        let mut merged = pull(10, &"a".repeat(40));
        *merged.get_mut("state").unwrap_or_else(|| unreachable!()) = serde_json::json!("closed");
        // GitHub's list route exposes merged_at, not necessarily merged.
        merged
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove("merged");
        merged
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert(
                "merged_at".into(),
                serde_json::json!("2026-11-13T00:00:00Z"),
            );
        let (endpoint, server) = serve(vec![(
            "200 OK",
            serde_json::json!([pull(9, &"a".repeat(40)), merged]).to_string(),
        )]);
        let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
            .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            client.ensure_pull_request(&plan()),
            Err(PortFailure::Unknown(_))
        ));
        assert!(
            server
                .join()
                .unwrap_or_else(|_| unreachable!())
                .iter()
                .all(|request| request.starts_with("GET "))
        );
    }
}
