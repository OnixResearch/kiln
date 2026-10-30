//! Authenticated, exact-revision code review with a private durable SQLite journal.
//! CI and source observations are shell facts, never accepted from request bodies.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kiln_adapter_git::change_review::{LocalGitReviewProfile, LocalGitReviewProvider};
use kiln_core::change_review as domain;
use kiln_provider_api::change_review::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::api::json_response;
use crate::engine::{ApiResponse, Role};

mod commands;
pub mod delivery;
pub mod intake;
mod model;
pub mod queue;
mod store;
mod suggestions;
use model::{Action, Request};

const MAX_BODY: usize = 128 * 1024;
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
const MAX_CHANGES: usize = 1000;
const MAX_PROFILE_BYTES: usize = 256 * 1024;
const MAX_PRINCIPALS: usize = 128;
const MAX_IDENTITY_BYTES: usize = 256;
const MAX_SHORT_TEXT_BYTES: usize = 128;
const MAX_TITLE_BYTES: usize = 1024;
const MAX_DESCRIPTION_BYTES: usize = 32 * 1024;
const MAX_COMMENT_BYTES: usize = 16 * 1024;
const MAX_QUERY_BYTES: usize = 1024;
const MAX_QUERY_FIELDS: usize = 16;
const MAX_TARGETS: usize = 32;
const REVIEW_CAPABILITY_COUNT: usize = 9;
const MAX_REQUEST_TARGET_BYTES: usize = 8192;
const MAX_DRAFTS: usize = 128;
const MAX_SAVED_VIEWS: usize = 64;
const MAX_DEPENDENCIES: usize = 64;
const MAX_REVISIONS: usize = 128;
const MAX_OPERATIONS: usize = 4000;
const MAX_ANCHOR_SPAN: u32 = 1000;
const RESERVATION_STATE_BYTES: usize = 12 * 1024 * 1024;
const CHANGE_PAGE_SIZE: usize = 25;
const NOTIFICATION_PAGE_SIZE: usize = 200;
const OPERATION_PAGE_SIZE: usize = 100;
const CI_RUNS_PER_REVISION: usize = 25;
const MILLIS_PER_SECOND: i64 = 1000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub database: PathBuf,
    pub project: String,
    pub provider: LocalGitReviewProfile,
    pub policy: domain::Policy,
    pub read_principals: Vec<String>,
    pub write_principals: Vec<String>,
    pub signature_name: String,
    pub signature_email: String,
    #[serde(default)]
    pub intake: Option<intake::IntakeProfile>,
    #[serde(default)]
    pub delivery: Option<delivery::Profile>,
    #[serde(default)]
    pub queue: Option<queue::Profile>,
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_PROFILE_BYTES, false)?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "review profile malformed")?;
        profile.admit()?;
        Ok(profile)
    }

    pub fn admit(&self) -> Result<(), String> {
        let readers: BTreeSet<_> = self.read_principals.iter().collect();
        let writers: BTreeSet<_> = self.write_principals.iter().collect();
        if self.schema != "kiln.code-review-profile.v1"
            || !valid_text(&self.project, MAX_IDENTITY_BYTES)
            || !valid_text(&self.policy.id, MAX_IDENTITY_BYTES)
            || readers.is_empty()
            || readers.len() > MAX_PRINCIPALS
            || readers.len() != self.read_principals.len()
            || writers.len() > MAX_PRINCIPALS
            || writers.len() != self.write_principals.len()
            || self
                .read_principals
                .iter()
                .chain(&self.write_principals)
                .any(|p| !valid_principal(p))
            || self.write_principals.iter().any(|p| !readers.contains(p))
            || self.policy.eligible_reviewers.len() > MAX_PRINCIPALS
            || self.policy.submitters.len() > MAX_PRINCIPALS
            || self
                .policy
                .eligible_reviewers
                .iter()
                .any(|p| !readers.contains(p) || !writers.contains(p))
            || self
                .policy
                .submitters
                .iter()
                .any(|p| !readers.contains(p) || !writers.contains(p))
            || self.policy.minimum_approvals == 0
            || self.policy.minimum_approvals > MAX_PRINCIPALS as u32
            || !valid_text(&self.signature_name, MAX_SHORT_TEXT_BYTES)
            || !valid_text(&self.signature_email, MAX_IDENTITY_BYTES)
            || !self.signature_email.contains('@')
            || self.signature_name.contains(['<', '>', '\n', '\r'])
            || self.signature_email.contains(['<', '>', '\n', '\r'])
        {
            return Err("review profile policy or principals rejected".into());
        }
        domain::validate_policy(&self.policy).map_err(|error| error.to_string())?;
        if let Some(intake) = &self.intake {
            intake.admit(&self.provider, &self.write_principals)?;
        }
        if let Some(delivery) = &self.delivery {
            delivery.admit(&self.read_principals)?;
        }
        if let Some(queue) = &self.queue {
            queue.admit(self)?;
        }
        store::preflight(&self.database)?;
        if !crate::profile::absolute(&self.provider.repository_path)
            || self.provider.target_refs.is_empty()
            || self.provider.target_refs.len() > MAX_TARGETS
            || self.provider.target_refs.iter().any(|reference| {
                !reference.starts_with("refs/heads/") || !git2::Reference::is_valid_name(reference)
            })
            || self.provider.capabilities.len() > REVIEW_CAPABILITY_COUNT
            || !self
                .provider
                .capabilities
                .contains(&ReviewCapability::Inspect)
            || !self
                .provider
                .capabilities
                .contains(&ReviewCapability::Compare)
        {
            return Err("review provider configuration rejected".into());
        }
        crate::worker::files::directory(&self.provider.repository_path)?;
        let canonical = self
            .provider
            .repository_path
            .canonicalize()
            .map_err(|_| "review repository unavailable")?;
        let identity = kiln_codec::identity_from_bytes(
            canonical
                .to_str()
                .ok_or("review repository path encoding")?
                .as_bytes(),
        )
        .map_err(|_| "review repository identity unavailable")?;
        if self.provider.repository_id != identity.as_str() {
            return Err("review repository identity mismatch".into());
        }
        // Adapter preflight only observes Git; it never initializes or writes refs.
        LocalGitReviewProvider::open(self.provider.clone())
            .map_err(|_| "review provider preflight rejected")?;
        Ok(())
    }

    pub fn admit_service(&self, service: &crate::profile::ServiceProfile) -> Result<(), String> {
        self.admit()?;
        let project = service
            .projects
            .iter()
            .find(|p| p.id == self.project)
            .ok_or("review CI project missing")?;
        let source = project
            .repository_path
            .canonicalize()
            .map_err(|_| "review CI repository unavailable")?;
        let provider = self
            .provider
            .repository_path
            .canonicalize()
            .map_err(|_| "review repository unavailable")?;
        if source != provider
            || self.provider.target_refs.iter().any(|reference| {
                reference != &project.reference && !project.admitted_references.contains(reference)
            })
        {
            return Err("review CI repository or target binding rejected".into());
        }
        Ok(())
    }

    fn binding(&self) -> Result<String, Failure> {
        hash(
            &json!({"schema":self.schema,"project":self.project,"repository":self.provider.repository_id,"targets":self.provider.target_refs,"intake":self.intake}),
        )
    }
    fn effective_policy(&self) -> Result<domain::Policy, Failure> {
        let mut policy = self.policy.clone();
        policy.id = hash(&policy)?;
        Ok(policy)
    }
}

fn valid_principal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
        && !value.contains('*')
}
fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn hash(value: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_vec(value)
        .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
        .map_err(|_| Failure::Unavailable)
}

#[derive(Debug)]
pub struct Workspace {
    profile: Option<Profile>,
    gate: Mutex<()>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct State {
    review: domain::ReviewState,
    versions: BTreeMap<String, u64>,
    operations: Vec<OperationView>,
    #[serde(default)]
    contributions: BTreeMap<String, intake::Contribution>,
    #[serde(default)]
    delivery: delivery::State,
    #[serde(default)]
    queue: Vec<queue::Entry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OperationView {
    change: String,
    operation_id: String,
    kind: String,
    state: String,
    result_change: Option<String>,
    target_ref: String,
    commit: Option<String>,
    target_commit: Option<String>,
}
#[derive(Debug)]
enum Failure {
    Api(u16, String),
    Domain(domain::ReviewError),
    Unavailable,
}
impl From<String> for Failure {
    fn from(_: String) -> Self {
        Self::Unavailable
    }
}
impl From<domain::ReviewError> for Failure {
    fn from(error: domain::ReviewError) -> Self {
        Self::Domain(error)
    }
}
impl From<ReviewError> for Failure {
    fn from(error: ReviewError) -> Self {
        let status = match error.kind {
            ReviewErrorKind::InvalidRequest => 400,
            ReviewErrorKind::Unsupported => 403,
            ReviewErrorKind::NotFound => 404,
            ReviewErrorKind::TargetMoved | ReviewErrorKind::Conflict => 409,
            _ => 503,
        };
        Self::Api(
            status,
            format!("review_provider_{:?}", error.kind).to_lowercase(),
        )
    }
}
fn reject<T>(status: u16, code: &str) -> Result<T, Failure> {
    Err(Failure::Api(status, code.into()))
}
fn response(value: Value) -> Result<ApiResponse, Failure> {
    json_response(200, value).map_err(Failure::from)
}

/// Authenticated shell observations held stable for one review request.
#[derive(Debug)]
pub struct RequestContext<'a> {
    pub role: Role,
    pub principal: &'a str,
    pub now_ms: i64,
    pub ci: &'a [domain::CiFact],
}

struct ProviderContext<'a, P: ChangeReviewPort> {
    profile: &'a Profile,
    provider: &'a P,
    binding: &'a str,
}

impl Workspace {
    pub(crate) fn deliver_notifications(&self, now_ms: i64) -> Result<(), String> {
        let Some(profile) = &self.profile else {
            return Ok(());
        };
        let Some(config) = &profile.delivery else {
            return Ok(());
        };
        // Only the serial polling executor calls this. Holding the review gate
        // spans reservation/delivery/completion, so another batch cannot classify
        // an in-flight Reserved attempt as crash uncertainty. Engine is not locked.
        let _gate = self
            .gate
            .lock()
            .map_err(|_| "review delivery lock unavailable")?;
        let binding = profile
            .binding()
            .map_err(|_| "review binding unavailable")?;
        let mut store = store::Store::open(&profile.database, &binding)?;
        let mut state: State = store.load()?;
        let work = delivery::reserve(config, &mut state.delivery, now_ms)?;
        store.commit(&binding, &state, None)?;
        for item in work {
            let completion = item.deliver()?;
            delivery::complete(&mut state.delivery, completion)?;
            store.commit(&binding, &state, None)?;
        }
        Ok(())
    }

    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self {
            profile,
            gate: Mutex::new(()),
        })
    }
    pub fn response(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        context: RequestContext<'_>,
    ) -> ApiResponse {
        match self.respond(method, target, body, &context) {
            Ok(response) if response.body.len() <= MAX_RESPONSE => response,
            Err(Failure::Api(status, code)) => {
                json_response(status, json!({"message":code.replace('_'," "),"code":code}))
                    .unwrap_or_else(|_| unavailable())
            }
            Err(Failure::Domain(error)) => {
                json_response(409, json!({"code":error.code,"message":error.message}))
                    .unwrap_or_else(|_| unavailable())
            }
            _ => unavailable(),
        }
    }
    fn respond(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        request_context: &RequestContext<'_>,
    ) -> Result<ApiResponse, Failure> {
        let RequestContext {
            role,
            principal,
            now_ms,
            ci,
        } = *request_context;
        if role == Role::Worker || !valid_principal(principal) {
            return reject(403, "review_principal_denied");
        }
        if target.len() > MAX_REQUEST_TARGET_BYTES || body.len() > MAX_BODY || now_ms < 0 {
            return reject(400, "review_request_invalid");
        }
        let (path, query) = parse_target(target)?;
        let detail_route = path
            .strip_prefix("/api/code-review/changes/")
            .is_some_and(|detail| {
                let id = detail.strip_suffix("/diff").unwrap_or(detail);
                id.len() == 64
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            });
        if !matches!(
            path.as_str(),
            "/api/code-review" | "/api/code-review/commands" | "/api/code-review/targets"
        ) && !detail_route
        {
            return reject(404, "route_missing");
        }
        let command_route = path == "/api/code-review/commands";
        if (command_route && method != "POST") || (!command_route && method != "GET") {
            return reject(405, "review_method_denied");
        }
        if !command_route && !body.is_empty() {
            return reject(400, "unexpected_body");
        }
        let Some(profile) = &self.profile else {
            return if path == "/api/code-review" {
                response(
                    json!({"schema":"kiln.code-review.v1","configured":false,"principal":principal,"can_write":false,"changes":[],"saved_views":[],"notifications":[],"capabilities":[]}),
                )
            } else {
                reject(404, "review_unconfigured")
            };
        };
        if !profile.read_principals.iter().any(|p| p == principal) {
            return reject(403, "review_read_denied");
        }
        let can_write = profile.write_principals.iter().any(|p| p == principal);
        if command_route && !can_write {
            return reject(403, "review_write_denied");
        }
        let request: Option<Request> = if command_route {
            Some(
                serde_json::from_slice(body)
                    .map_err(|_| Failure::Api(400, "review_command_invalid".into()))?,
            )
        } else {
            None
        };
        let _gate = self.gate.lock().map_err(|_| Failure::Unavailable)?;
        let binding = profile.binding()?;
        let mut store = store::Store::open(&profile.database, &binding)?;
        let mut state: State = store.load()?;
        if state.review.changes.len() > MAX_CHANGES
            || state
                .review
                .changes
                .values()
                .any(|c| c.revisions.is_empty())
        {
            return Err(Failure::Unavailable);
        }
        let provider = LocalGitReviewProvider::open(profile.provider.clone())?;
        let context = ProviderContext {
            profile,
            provider: &provider,
            binding: &binding,
        };
        self.recover(&context, &mut store, &mut state)?;
        if let Some(request) = request {
            return self.command(&context, &mut store, &mut state, request, request_context);
        }
        let projection = domain::project(&state.review, principal);
        if path == "/api/code-review" {
            let query_filter = search_query(&query)?;
            let offset = query
                .get("offset")
                .map(|s| s.parse::<usize>())
                .transpose()
                .map_err(|_| Failure::Api(400, "review_query_invalid".into()))?
                .unwrap_or(0);
            let found = domain::search(&state.review, principal, &query_filter);
            let total = found.len();
            let changes: Vec<_> = found
                .into_iter()
                .skip(offset)
                .take(CHANGE_PAGE_SIZE)
                .map(|c| summary(c, &state))
                .collect();
            let next_offset = (offset.saturating_add(changes.len()) < total)
                .then_some(offset.saturating_add(changes.len()));
            let notification_offset = page_offset(&query, "notification_offset")?;
            let mut inbox: Vec<_> = projection
                .inbox
                .iter()
                .chain(delivery::inbox(&state.delivery, principal))
                .collect();
            inbox.sort_by(|a, b| {
                b.created_ms
                    .cmp(&a.created_ms)
                    .then_with(|| a.id.cmp(&b.id))
            });
            let notifications_total = inbox.len();
            let notifications: Vec<_> = inbox
                .into_iter()
                .skip(notification_offset)
                .take(NOTIFICATION_PAGE_SIZE)
                .collect();
            let next_notification_offset =
                (notification_offset.saturating_add(notifications.len()) < notifications_total)
                    .then_some(notification_offset.saturating_add(notifications.len()));
            let operation_offset = page_offset(&query, "operation_offset")?;
            let pending: Vec<_> = state
                .operations
                .iter()
                .filter(|op| matches!(op.state.as_str(), "pending" | "uncertain"))
                .collect();
            let operations: Vec<_> = pending
                .iter()
                .skip(operation_offset)
                .take(OPERATION_PAGE_SIZE)
                .collect();
            let next_operation_offset = (operation_offset.saturating_add(operations.len())
                < pending.len())
            .then_some(operation_offset.saturating_add(operations.len()));
            return response(
                json!({"schema":"kiln.code-review.v1","configured":true,"principal":principal,"can_write":can_write,
                "project":profile.project,"repository":profile.provider.repository_id,"target_refs":profile.provider.target_refs,
                "capabilities":provider.info().capabilities,"policy":profile.policy,"changes":changes,"total":total,"next_offset":next_offset,
                "saved_views":projection.saved_queries,"notifications":notifications,"notifications_total":notifications_total,
                "next_notification_offset":next_notification_offset,"operations":operations,"operations_total":pending.len(),"next_operation_offset":next_operation_offset,
                "contribution_sources":intake::sources(profile,&state,principal)?,"intake_enabled":profile.intake.is_some(),
                "queue":state.queue,"queue_enabled":profile.queue.is_some(),
                "target_write_boundary":{"scope":"configured_provider_only","targets":profile.provider.target_refs,"unmanaged_writers":"Operator must restrict repository filesystem and other push credentials; service admission does not control external writers"},
                "bounded":true,"non_claims":["External delivery reports receiver acceptance, not email receipt.","Provider-created commits use the service signature, not an attestation of the original author's identity.","Configured provider policy does not prevent unmanaged repository writers."]}),
            );
        }
        if path == "/api/code-review/targets" {
            let commit = query
                .get("commit")
                .ok_or_else(|| Failure::Api(400, "review_commit_required".into()))?;
            if !state
                .review
                .changes
                .values()
                .any(|c| c.revisions.iter().any(|r| &r.commit == commit))
            {
                return reject(404, "review_revision_missing");
            }
            let target = query
                .get("target_ref")
                .ok_or_else(|| Failure::Api(400, "review_target_required".into()))?;
            self.require_target(profile, target)?;
            let inspection = provider.inspect(commit, target)?;
            return response(
                json!({"source":source_fact(profile,&inspection,inspection.target_commit.clone())}),
            );
        }
        let Some(detail) = path.strip_prefix("/api/code-review/changes/") else {
            return reject(404, "route_missing");
        };
        let (id, diff) = match detail.strip_suffix("/diff") {
            Some(id) => (id, true),
            None => (detail, false),
        };
        let change = state
            .review
            .changes
            .get(id)
            .ok_or_else(|| Failure::Api(404, "review_change_missing".into()))?;
        if diff {
            return self.diff(&provider, change, &query);
        }
        let facts = self.facts(&context, &state, change, now_ms, ci, "observation")?;
        let checklist = domain::checklist(&state.review, id, &facts)?;
        let dependencies: Vec<_> = change
            .dependencies
            .iter()
            .filter_map(|d| state.review.changes.get(&d.change))
            .map(|c| summary(c, &state))
            .collect();
        let dependents: Vec<_> = state
            .review
            .changes
            .values()
            .filter(|c| c.dependencies.iter().any(|d| d.change == id))
            .map(|c| summary(c, &state))
            .collect();
        let drafts: Vec<_> = projection
            .drafts
            .iter()
            .filter(|d| d.change == id)
            .collect();
        let operations: Vec<_> = state
            .operations
            .iter()
            .filter(|op| op.change == id || op.result_change.as_deref() == Some(id))
            .collect();
        let commits: BTreeSet<_> = change
            .revisions
            .iter()
            .map(|revision| revision.commit.as_str())
            .collect();
        let mut matched_ci: Vec<_> = ci
            .iter()
            .filter(|fact| {
                fact.project == change.project
                    && fact.repository == change.repository
                    && commits.contains(fact.revision.as_str())
            })
            .collect();
        matched_ci.sort_by(|a, b| b.ordinal.cmp(&a.ordinal));
        let ci_runs_total = matched_ci.len();
        let mut retained: BTreeMap<&str, usize> = BTreeMap::new();
        let ci_runs: Vec<_> = matched_ci
            .into_iter()
            .filter(|fact| {
                let count = retained.entry(fact.revision.as_str()).or_default();
                *count += 1;
                *count <= CI_RUNS_PER_REVISION
            })
            .collect();
        response(
            json!({"change":enriched(change,&state,principal),"drafts":drafts,"checklist":checklist,"source":facts.source,"stack":{"dependencies":dependencies,"dependents":dependents},"operations":operations,"ci_runs_truncated":ci_runs.len() < ci_runs_total,"ci_runs":ci_runs,"ci_runs_total":ci_runs_total,"ci_runs_bounded":true,"ci_runs_per_revision_limit":CI_RUNS_PER_REVISION,
                "subscribed":delivery::subscribed(&state.delivery,principal,id),"delivery_outcomes":delivery::outcomes(&state.delivery,principal,id),
                "queue":state.queue.iter().rev().find(|entry|entry.change==id),"queue_enabled":profile.queue.is_some()}),
        )
    }

    fn require_target(&self, profile: &Profile, target: &str) -> Result<(), Failure> {
        if !profile
            .provider
            .target_refs
            .iter()
            .any(|allowed| allowed == target)
        {
            return reject(403, "review_target_denied");
        }
        Ok(())
    }
    fn facts(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        state: &State,
        change: &domain::Change,
        now_ms: i64,
        ci: &[domain::CiFact],
        event_id: &str,
    ) -> Result<domain::Facts, Failure> {
        let ProviderContext {
            profile, provider, ..
        } = *context;
        let revision = change.current()?;
        let inspection = provider.inspect(&revision.commit, &change.target_ref)?;
        let mut source = source_fact(profile, &inspection, revision.base.clone());
        for dependency in &change.dependencies {
            if let Some(revision) = state
                .review
                .changes
                .get(&dependency.change)
                .and_then(|c| c.revisions.iter().find(|r| r.id == dependency.revision))
            {
                let observed = provider.inspect(&revision.commit, &change.target_ref)?;
                if observed.target_commit != inspection.target_commit {
                    return reject(409, "review_target_moved");
                }
                if observed.ancestor_of_target {
                    source.target_ancestors.insert(revision.commit.clone());
                }
            }
        }
        let integration =
            queue::integration(profile, provider, state, change, &inspection.target_commit)?;
        let tested_commit = integration
            .as_ref()
            .map_or(revision.commit.as_str(), |proof| {
                proof.candidate_commit.as_str()
            });
        let ci = ci
            .iter()
            .filter(|fact| {
                fact.project == change.project
                    && fact.repository == change.repository
                    && fact.revision == tested_commit
                    && fact.full_run
                    && fact.clean
                    && !fact.run_id.is_empty()
            })
            .max_by_key(|fact| fact.ordinal)
            .cloned()
            .into_iter()
            .collect();
        let mut facts = domain::Facts {
            now_ms,
            event_id: event_id.into(),
            policy: Some(profile.effective_policy()?),
            source: Some(source),
            ci,
            anchors: Vec::new(),
            integration,
        };
        self.populate_changed_files(provider, revision, &mut facts)?;
        Ok(facts)
    }
    fn populate_changed_files(
        &self,
        provider: &impl ChangeReviewPort,
        revision: &domain::Revision,
        facts: &mut domain::Facts,
    ) -> Result<(), Failure> {
        let comparison = provider.compare(&CompareRequest {
            old_commit: revision.base.clone(),
            new_commit: revision.commit.clone(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })?;
        let complete = !comparison.truncated && comparison.files.iter().all(|file| !file.truncated);
        let files = comparison
            .files
            .into_iter()
            .filter_map(|file| {
                let path = file.new_path.clone().or_else(|| file.old_path.clone())?;
                Some(domain::ChangedFile {
                    path,
                    old_path: file.old_path,
                })
            })
            .collect();
        if let Some(source) = facts.source.as_mut() {
            source.changed_files = Some(domain::ChangedFiles { files, complete });
        }
        Ok(())
    }

    fn record_transition(
        &self,
        profile: &Profile,
        before: &State,
        after: &mut State,
        actor: &str,
        facts: &domain::Facts,
    ) -> Result<(), Failure> {
        delivery::after_transition(
            &before.review,
            &after.review,
            &mut after.delivery,
            profile.delivery.as_ref(),
            &profile.read_principals,
            actor,
            facts,
        )
        .map_err(Failure::from)
    }

    fn diff(
        &self,
        provider: &impl ChangeReviewPort,
        change: &domain::Change,
        query: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, Failure> {
        // r[impl kiln.highlight.api]
        let highlight = match query.get("highlight").map(String::as_str) {
            None => false,
            Some("1") => true,
            Some(_) => return reject(400, "review_query_invalid"),
        };
        let revision = match query.get("revision") {
            Some(id) => change
                .revisions
                .iter()
                .find(|r| &r.id == id)
                .ok_or_else(|| Failure::Api(404, "review_revision_missing".into()))?,
            None => change.current()?,
        };
        let old = match query.get("base").map(String::as_str) {
            None | Some("base") => &revision.base,
            Some(id) => {
                &change
                    .revisions
                    .iter()
                    .find(|r| r.id == id)
                    .ok_or_else(|| Failure::Api(404, "review_base_missing".into()))?
                    .commit
            }
        };
        let mut diff = provider.compare(&CompareRequest {
            old_commit: old.clone(),
            new_commit: revision.commit.clone(),
            limits: DiffLimits::default(),
            whitespace: match query.get("whitespace").map(String::as_str) {
                None | Some("exact") => WhitespaceMode::Exact,
                Some("ignore_all") => WhitespaceMode::IgnoreAll,
                Some("ignore_change") => WhitespaceMode::IgnoreChange,
                Some("ignore_eol") => WhitespaceMode::IgnoreEol,
                _ => return reject(400, "review_whitespace_invalid"),
            },
        })?;
        if let Some(file) = query.get("file") {
            diff.files
                .retain(|f| f.new_path.as_ref() == Some(file) || f.old_path.as_ref() == Some(file));
            if diff.files.is_empty() {
                return reject(404, "review_file_missing");
            }
        }
        let mut value = serde_json::to_value(&diff).map_err(|_| Failure::Unavailable)?;
        if !highlight {
            return response(value);
        }
        let attach = |value: &mut Value, highlighted: kiln_highlight::Highlight| {
            let encoded = serde_json::to_value(highlighted).map_err(|_| Failure::Unavailable)?;
            if let Some(object) = value.as_object_mut() {
                object.insert("highlight".into(), encoded);
            }
            Ok::<(), Failure>(())
        };
        attach(&mut value, highlight_comparison(&diff))?;
        let answer = json_response(200, &value).map_err(Failure::from)?;
        if answer.body.len() <= MAX_RESPONSE {
            return Ok(answer);
        }
        // Highlighting never turns a bounded diff into an unavailable response.
        attach(
            &mut value,
            kiln_highlight::Highlight::skipped(None, kiln_highlight::Reason::TooLarge),
        )?;
        response(value)
    }
}

/// Highlight a comparison: `lines[i]` is the `i`-th row of every hunk of
/// every returned file, in order, with offsets relative to the row's `text`.
fn highlight_comparison(diff: &CommitComparison) -> kiln_highlight::Highlight {
    let files: Vec<kiln_highlight::DiffFile<'_>> = diff
        .files
        .iter()
        .map(|file| kiln_highlight::DiffFile {
            old_path: file.old_path.as_deref(),
            new_path: file.new_path.as_deref(),
            binary: file.binary,
            hunks: file
                .hunks
                .iter()
                .map(|hunk| {
                    hunk.lines
                        .iter()
                        .map(|line| kiln_highlight::DiffRow {
                            side: match line.kind {
                                DiffLineKind::Context => kiln_highlight::DiffSide::Both,
                                DiffLineKind::Deletion => kiln_highlight::DiffSide::Old,
                                DiffLineKind::Addition => kiln_highlight::DiffSide::New,
                                DiffLineKind::NoNewline => kiln_highlight::DiffSide::Neither,
                            },
                            text: &line.text,
                        })
                        .collect()
                })
                .collect(),
        })
        .collect();
    kiln_highlight::highlight_diff_files(&files, &kiln_highlight::Bounds::DEFAULT)
}

fn source_fact(
    profile: &Profile,
    inspection: &CommitInspection,
    base: String,
) -> domain::SourceFact {
    domain::SourceFact {
        project: profile.project.clone(),
        repository: inspection.repository_id.clone(),
        target_ref: inspection.target_ref.clone(),
        commit: inspection.commit.clone(),
        base,
        current_target: inspection.target_commit.clone(),
        immutable: true,
        target_contains_commit: inspection.ancestor_of_target,
        target_is_ancestor: inspection.target_is_ancestor,
        target_ancestors: BTreeSet::new(),
        changed_files: None,
    }
}
fn enriched(change: &domain::Change, state: &State, principal: &str) -> Value {
    let reviewed: Vec<_> = domain::reviewed(change, principal).collect();
    json!({"id":change.id,"version":state.versions.get(&change.id).copied().unwrap_or(0),
        "project":change.project,"repository":change.repository,"target_ref":change.target_ref,
        "owner":change.owner,"title":change.title,"description":change.description,"state":change.state,
        "wip":change.wip,"revisions":change.revisions,"reviewers":change.reviewers,"votes":change.votes,
        "threads":change.threads,"reviewed":reviewed,"attention":change.attention,
        "dependencies":change.dependencies,"candidate":change.candidate,"submission":change.submission,
        "maintenance":change.maintenance})
}
fn summary(change: &domain::Change, state: &State) -> Value {
    let revision = change.revisions.last().map(|revision|json!({"id":revision.id,"number":revision.number,"commit":revision.commit,"base":revision.base,"author":revision.author,"created_ms":revision.created_ms}));
    json!({"id":change.id,"version":state.versions.get(&change.id).copied().unwrap_or(0),"project":change.project,"repository":change.repository,"target_ref":change.target_ref,"owner":change.owner,"title":change.title,"state":change.state,"wip":change.wip,"revisions":revision.into_iter().collect::<Vec<_>>(),"reviewers":change.reviewers,"attention":change.attention,"unresolved":change.threads.iter().filter(|t| !t.resolved).count()})
}
fn search_query(query: &BTreeMap<String, String>) -> Result<domain::SearchQuery, Failure> {
    let kind = match query.get("view").map_or("all", String::as_str) {
        "all" => domain::SearchKind::All,
        "awaiting_review" => domain::SearchKind::AwaitingReview,
        "owned_needs_revision" => domain::SearchKind::OwnedNeedsRevision,
        "unresolved" => domain::SearchKind::Unresolved,
        _ => return reject(400, "review_query_invalid"),
    };
    let status = match query.get("status").map(String::as_str) {
        None | Some("") => None,
        Some("open") => Some(domain::ChangeStatus::Open),
        Some("abandoned") => Some(domain::ChangeStatus::Abandoned),
        Some("submitted") => Some(domain::ChangeStatus::Submitted),
        _ => return reject(400, "review_query_invalid"),
    };
    Ok(domain::SearchQuery {
        kind,
        text: query.get("q").cloned().unwrap_or_default(),
        status,
    })
}
fn parse_target(target: &str) -> Result<(String, BTreeMap<String, String>), Failure> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut result = BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode(key)?;
        let value = decode(value)?;
        if result.insert(key, value).is_some() || result.len() > MAX_QUERY_FIELDS {
            return reject(400, "review_query_invalid");
        }
    }
    Ok((path.into(), result))
}
fn decode(value: &str) -> Result<String, Failure> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut source = value.bytes();
    while let Some(byte) = source.next() {
        match byte {
            b'%' => {
                let high = source
                    .next()
                    .and_then(|b| char::from(b).to_digit(16))
                    .ok_or_else(|| Failure::Api(400, "review_query_invalid".into()))?;
                let low = source
                    .next()
                    .and_then(|b| char::from(b).to_digit(16))
                    .ok_or_else(|| Failure::Api(400, "review_query_invalid".into()))?;
                bytes.push((high * 16 + low) as u8);
            }
            b'+' => bytes.push(b' '),
            byte => bytes.push(byte),
        }
    }
    String::from_utf8(bytes).map_err(|_| Failure::Api(400, "review_query_invalid".into()))
}
fn unavailable() -> ApiResponse {
    ApiResponse {status:503,content_type:"application/json",body:br#"{"code":"review_store_unavailable","message":"Review storage is unavailable, busy, unsafe or exceeded its admitted bounds"}"#.to_vec(),download_name:None}
}
fn page_offset(query: &BTreeMap<String, String>, key: &str) -> Result<usize, Failure> {
    query
        .get(key)
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| Failure::Api(400, "review_query_invalid".into()))
        .map(|value| value.unwrap_or(0))
}
