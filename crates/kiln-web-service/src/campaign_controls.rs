//! Authenticated campaign controls over the canonical campaign owner.
//!
//! Start previews and the decision inbox are read-only. Commands are typed
//! intents that the canonical owner applies with its durable command record.
//! This shell authorizes them against a control profile that it rereads for
//! every request, so a removed or expired grant stops working at once.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::implementation::campaign::{
    CampaignContract, CampaignState, CompletionGoal, DecisionContextBinding, DecisionKind,
    DecisionRequestState, HumanDecisionRequest, MAX_ID_BYTES, MemberBinding, MemberSelector,
    WorkflowProfile, selector_key,
};
use kiln_core::types::ContentId;
use kiln_implementer::campaign::{
    AdmissionState, AppliedCommand, CampaignObservation, CampaignStore, DecisionOutcome,
    OperatorCommand, OperatorCommandKind, OperatorFailure, OperatorIntent, valid_request_id,
};
use serde::{Deserialize, Serialize};

use crate::engine::{ApiResponse, Role};
use crate::implementation_campaign::{NativeDeliveryRoute, route_supports_goal};

pub const PROFILE_SCHEMA: &str = "kiln.web-campaign-controls.v1";
pub const DEFINITIONS_SCHEMA: &str = "kiln.campaign-definitions.v1";
pub const INBOX_SCHEMA: &str = "kiln.campaign-inbox.v1";
pub const RESULT_SCHEMA: &str = "kiln.campaign-command-result.v1";
pub const DEFINITIONS_ROUTE: &str = "/api/implementation/campaign-definitions";
pub const INBOX_ROUTE: &str = "/api/implementation/campaigns/inbox";
pub const COMMANDS_ROUTE: &str = "/api/implementation/campaigns/commands";
pub const ROUTES: [&str; 3] = [DEFINITIONS_ROUTE, INBOX_ROUTE, COMMANDS_ROUTE];

const MAX_PROFILE_BYTES: usize = 1_048_576;
const MAX_CONTRACT_BYTES: usize = 4 * 1_048_576;
const MAX_COMMAND_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1_048_576;
const MAX_GRANTS: usize = 128;
const MAX_GRANT_CAMPAIGNS: usize = 64;
const MAX_REVIEW_ROLES: usize = 32;
const MAX_ROLE_BYTES: usize = 128;
const MAX_PRINCIPAL_BYTES: usize = 256;
const MAX_DEFINITIONS: usize = 64;
const GOALS: [CompletionGoal; 4] = [
    CompletionGoal::VerifiedCandidate,
    CompletionGoal::VerifiedPullRequest,
    CompletionGoal::IntegratedChange,
    CompletionGoal::NativeLifecycleComplete,
];
const NON_CLAIMS: [&str; 4] = [
    "a preview, inbox or command result grants no claim, dispatch, delivery or completion",
    "pause stops successor admission and decision issuance; running attempts and workers are not stopped",
    "cancellation does not observe worker termination, provider cleanup or refunded usage",
    "profile grants are listed principals, not holder-bound capabilities",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub schema: String,
    pub campaign_database: PathBuf,
    pub grants: Vec<Grant>,
    #[serde(default)]
    pub definitions: Vec<Definition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Grant {
    pub principal: String,
    pub campaigns: Vec<String>,
    pub commands: Vec<OperatorCommandKind>,
    #[serde(default)]
    pub review_roles: Vec<String>,
    /// The grant admits nothing at or after this host time.
    #[serde(default)]
    pub not_after_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Definition {
    pub contract: PathBuf,
    pub delivery_route: NativeDeliveryRoute,
}

fn valid_principal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PRINCIPAL_BYTES
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && !value.contains('*')
}

fn valid_campaign_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_ID_BYTES && !value.chars().any(char::is_control)
}

fn valid_role(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ROLE_BYTES
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && !value.contains('*')
}

fn unique<'a, T: Ord + 'a>(values: impl IntoIterator<Item = &'a T>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

impl Profile {
    fn admit(&self) -> Result<(), &'static str> {
        let grants_valid = (1..=MAX_GRANTS).contains(&self.grants.len())
            && unique(self.grants.iter().map(|grant| &grant.principal))
            && self.grants.iter().all(|grant| {
                valid_principal(&grant.principal)
                    && (1..=MAX_GRANT_CAMPAIGNS).contains(&grant.campaigns.len())
                    && grant.campaigns.iter().all(|id| valid_campaign_id(id))
                    && unique(&grant.campaigns)
                    && !grant.commands.is_empty()
                    && unique(&grant.commands)
                    && grant.review_roles.len() <= MAX_REVIEW_ROLES
                    && grant.review_roles.iter().all(|role| valid_role(role))
                    && unique(&grant.review_roles)
            });
        let definitions_valid = self.definitions.len() <= MAX_DEFINITIONS
            && self
                .definitions
                .iter()
                .all(|definition| crate::profile::absolute(&definition.contract))
            && unique(
                self.definitions
                    .iter()
                    .map(|definition| &definition.contract),
            );
        if self.schema != PROFILE_SCHEMA
            || !crate::profile::absolute(&self.campaign_database)
            || !grants_valid
            || !definitions_valid
        {
            return Err("campaign control profile rejected");
        }
        Ok(())
    }

    fn load(path: &Path) -> Result<Self, &'static str> {
        let bytes = crate::worker::files::regular(path, MAX_PROFILE_BYTES, false)
            .map_err(|_| "campaign control profile unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "campaign control profile malformed")?;
        profile.admit()?;
        Ok(profile)
    }

    /// The caller's grant, if it is still current.
    fn grant(&self, principal: &str, now_ms: u64) -> Option<&Grant> {
        self.grants.iter().find(|grant| {
            grant.principal == principal && grant.not_after_ms.is_none_or(|limit| now_ms < limit)
        })
    }
}

impl Grant {
    fn permits(&self, campaign_id: &str, command: OperatorCommandKind) -> bool {
        self.campaigns.iter().any(|id| id == campaign_id) && self.commands.contains(&command)
    }

    /// A required review needs one of the member's review roles; a member that
    /// declares none admits any principal granted the command.
    fn in_audience(&self, required_reviews: &[String]) -> bool {
        required_reviews.is_empty()
            || required_reviews
                .iter()
                .any(|role| self.review_roles.contains(role))
    }
}

/// One frozen definition read for this request.
struct LoadedDefinition {
    contract: CampaignContract,
    route: NativeDeliveryRoute,
    identity: ContentId,
}

impl LoadedDefinition {
    fn unsupported_goal(&self) -> bool {
        self.contract
            .members
            .iter()
            .any(|member| !route_supports_goal(self.route, member.goal))
    }
}

fn read_definitions(profile: &Profile) -> Result<Vec<LoadedDefinition>, &'static str> {
    let mut campaign_ids = BTreeSet::new();
    profile
        .definitions
        .iter()
        .map(|definition| {
            let bytes =
                crate::worker::files::regular(&definition.contract, MAX_CONTRACT_BYTES, false)
                    .map_err(|_| "campaign definition unavailable or unsafe")?;
            let contract: CampaignContract =
                serde_json::from_slice(&bytes).map_err(|_| "campaign definition malformed")?;
            let identity = contract
                .identity()
                .map_err(|_| "campaign definition invalid")?;
            if !campaign_ids.insert(contract.campaign_id.clone()) {
                return Err("campaign definitions repeat a campaign ID");
            }
            Ok(LoadedDefinition {
                contract,
                route: definition.delivery_route,
                identity,
            })
        })
        .collect()
}

fn same_database(control: &Path, observation: &Path) -> Result<(), String> {
    let control = std::fs::canonicalize(control)
        .map_err(|_| "campaign control database unavailable".to_owned())?;
    let observation = std::fs::canonicalize(observation)
        .map_err(|_| "displayed campaign database unavailable".to_owned())?;
    if control != observation {
        return Err(
            "campaign control database differs from the --implementation-work database".into(),
        );
    }
    Ok(())
}

/// One authenticated control request.
#[derive(Clone, Copy, Debug)]
pub struct ControlRequest<'a> {
    pub method: &'a str,
    pub target: &'a str,
    pub body: &'a [u8],
    pub principal: &'a str,
    pub role: Role,
    pub now_ms: u64,
}

/// A response plus the denial code a server records in its audit.
#[derive(Clone, Debug)]
pub struct ControlResponse {
    pub response: ApiResponse,
    pub denial: Option<&'static str>,
}

/// Service-side controls. Nothing here outlives one request but two paths.
#[derive(Clone, Debug)]
pub struct Controls {
    profile: PathBuf,
    observation: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Definitions,
    Inbox,
    Commands,
}

impl Controls {
    /// Admit the profile and its definitions at startup. `observation` is the
    /// `--implementation-work` database the console displays; the controls
    /// must name the same file.
    pub fn new(profile: &Path, observation: Option<&Path>) -> Result<Self, String> {
        let observation = observation.ok_or("campaign controls require --implementation-work")?;
        let loaded = Profile::load(profile).map_err(str::to_owned)?;
        same_database(&loaded.campaign_database, observation)?;
        read_definitions(&loaded).map_err(str::to_owned)?;
        Ok(Self {
            profile: profile.to_owned(),
            observation: observation.to_owned(),
        })
    }

    fn current_profile(&self) -> Result<Profile, ()> {
        let profile = Profile::load(&self.profile).map_err(|_| ())?;
        same_database(&profile.campaign_database, &self.observation).map_err(|_| ())?;
        Ok(profile)
    }

    // r[impl kiln.implementation.campaign.controls.scenario.roles]
    pub fn response(&self, request: &ControlRequest<'_>) -> ControlResponse {
        if request.role == Role::Worker {
            return denied("campaign_controls_forbidden");
        }
        let (path, query) = request
            .target
            .split_once('?')
            .map_or((request.target, None), |(path, query)| (path, Some(query)));
        let route = match path {
            DEFINITIONS_ROUTE => Route::Definitions,
            INBOX_ROUTE => Route::Inbox,
            COMMANDS_ROUTE => Route::Commands,
            _ => return failure(404, "route_missing"),
        };
        let method = if route == Route::Commands {
            "POST"
        } else {
            "GET"
        };
        if request.method != method {
            return failure(405, "campaign_controls_method_denied");
        }
        if route != Route::Commands && !request.body.is_empty() {
            return failure(400, "unexpected_body");
        }
        if route != Route::Inbox && query.is_some() {
            return failure(400, "campaign_controls_query_invalid");
        }
        if route == Route::Commands && request.role != Role::Operator {
            return denied("campaign_command_denied");
        }
        let Ok(profile) = self.current_profile() else {
            return failure(503, "campaign_controls_unavailable");
        };
        match route {
            Route::Definitions => definitions(&profile, request),
            Route::Inbox => match query.map(inbox_campaign) {
                Some(Ok(campaign_id)) => inbox(&profile, request, &campaign_id),
                _ => failure(400, "campaign_controls_query_invalid"),
            },
            Route::Commands => command(&profile, request),
        }
    }
}

fn inbox_campaign(query: &str) -> Result<String, ()> {
    // Each percent-encoded UTF-8 byte occupies three query bytes (%HH).
    let value = query
        .strip_prefix("campaign=")
        .filter(|value| value.len() <= MAX_ID_BYTES * 3 && !value.contains('&'))
        .ok_or(())?;
    let identity = crate::engine::api::decoded(value)?;
    if !valid_campaign_id(&identity) {
        return Err(());
    }
    Ok(identity)
}

fn respond(status: u16, body: &impl Serialize) -> ControlResponse {
    match serde_json::to_vec(body) {
        Ok(body) if body.len() <= MAX_RESPONSE_BYTES => ControlResponse {
            response: ApiResponse {
                status,
                content_type: "application/json",
                body,
                download_name: None,
                headers: Vec::new(),
            },
            denial: None,
        },
        _ => failure(503, "campaign_controls_byte_bound"),
    }
}

/// What the operator needs to know about a refusal. The console adds what
/// happened to the request itself.
fn message(code: &str) -> &'static str {
    match code {
        "campaign_version_conflict"
        | "campaign_definition_changed"
        | "campaign_decision_stale"
        | "campaign_decision_scope_changed" => {
            "Campaign state changed since it was displayed; reload and review."
        }
        "campaign_request_conflict" => "This request ID was already used for another command.",
        "campaign_command_denied" | "campaign_controls_forbidden" => {
            "No current grant admits this command for this principal."
        }
        "campaign_endpoint_unsupported" => {
            "A member's completion goal is not offered by the definition's delivery route."
        }
        "campaign_exists" => {
            "The campaign already exists; a changed selection needs an explicit owner revision."
        }
        "campaign_admission_closed" => "Campaign admission is paused or cancelled.",
        "campaign_decision_queue_full" => "The shared outstanding decision bound is full.",
        "campaign_decision_unknown" => "The decision is not retained by this campaign.",
        "campaign_not_found" => "The campaign is not in the canonical store.",
        _ => "Request could not be admitted",
    }
}

fn failure(status: u16, code: &'static str) -> ControlResponse {
    ControlResponse {
        response: ApiResponse {
            status,
            content_type: "application/json",
            body: serde_json::json!({"code": code, "message": message(code)})
                .to_string()
                .into_bytes(),
            download_name: None,
            headers: Vec::new(),
        },
        denial: None,
    }
}

fn denied(code: &'static str) -> ControlResponse {
    ControlResponse {
        denial: Some(code),
        ..failure(403, code)
    }
}

#[derive(Serialize)]
struct MemberPreview<'a> {
    member: String,
    change: &'a str,
    tasks: &'a [String],
    prerequisite_inclusion: bool,
    profile: &'a WorkflowProfile,
    profile_version: &'a ContentId,
    policy: &'a ContentId,
    repository_instructions: &'a ContentId,
    integration_policy: &'a ContentId,
    goal: CompletionGoal,
    goal_supported: bool,
    required_checks: &'a [String],
    required_reviews: &'a [String],
    context: &'a [DecisionContextBinding],
}

#[derive(Serialize)]
struct ExistingCampaign {
    store_version: u64,
    selection_identity: ContentId,
    same_selection: bool,
}

#[derive(Serialize)]
struct DefinitionPreview<'a> {
    definition_identity: &'a ContentId,
    campaign_id: &'a str,
    revision: u64,
    repository: &'a str,
    delivery_route: NativeDeliveryRoute,
    offered_goals: Vec<CompletionGoal>,
    members: Vec<MemberPreview<'a>>,
    max_concurrency: u16,
    max_outstanding_decisions: u16,
    parent_budget_id: &'a ContentId,
    agent_definition: &'a ContentId,
    require_one_writer_per_repository: bool,
    existing: Option<ExistingCampaign>,
    can_start: bool,
    refusals: Vec<&'static str>,
}

#[derive(Serialize)]
struct DefinitionsResponse<'a> {
    schema: &'static str,
    principal: &'a str,
    definitions: Vec<DefinitionPreview<'a>>,
    non_claims: [&'static str; 4],
}

fn member_preview(binding: &MemberBinding, route: NativeDeliveryRoute) -> MemberPreview<'_> {
    let (change, tasks): (&str, &[String]) = match &binding.selector {
        MemberSelector::WholeChange { change } => (change, &[]),
        MemberSelector::Tasks { change, tasks } => (change, tasks),
    };
    MemberPreview {
        member: selector_key(&binding.selector),
        change,
        tasks,
        prerequisite_inclusion: binding.prerequisite_inclusion,
        profile: &binding.profile,
        profile_version: &binding.profile_version,
        policy: &binding.policy,
        repository_instructions: &binding.repository_instructions,
        integration_policy: &binding.integration_policy,
        goal: binding.goal,
        goal_supported: route_supports_goal(route, binding.goal),
        required_checks: &binding.required_checks,
        required_reviews: &binding.required_reviews,
        context: &binding.context,
    }
}

// r[impl kiln.implementation.campaign.controls.scenario.start]
// r[impl kiln.implementation.campaign.controls.scenario.endpoints]
fn definitions(profile: &Profile, request: &ControlRequest<'_>) -> ControlResponse {
    let Ok(loaded) = read_definitions(profile) else {
        return failure(503, "campaign_controls_unavailable");
    };
    let Ok(observation) = CampaignObservation::open(&profile.campaign_database) else {
        return failure(503, "campaign_store_unavailable");
    };
    let grant = (request.role == Role::Operator)
        .then(|| profile.grant(request.principal, request.now_ms))
        .flatten();
    let mut previews = Vec::with_capacity(loaded.len());
    for definition in &loaded {
        let contract = &definition.contract;
        let Ok(existing) = observation.inspect(&contract.campaign_id) else {
            return failure(503, "campaign_store_unavailable");
        };
        let existing = existing.map(|(state, store_version)| ExistingCampaign {
            store_version,
            same_selection: state.selection_identity == definition.identity,
            selection_identity: state.selection_identity,
        });
        let mut refusals = Vec::new();
        if !grant
            .is_some_and(|grant| grant.permits(&contract.campaign_id, OperatorCommandKind::Start))
        {
            refusals.push("start_not_granted");
        }
        if existing.is_some() {
            refusals.push("campaign_exists");
        }
        if definition.unsupported_goal() {
            refusals.push("endpoint_unsupported");
        }
        previews.push(DefinitionPreview {
            definition_identity: &definition.identity,
            campaign_id: &contract.campaign_id,
            revision: contract.revision,
            repository: &contract.repository,
            delivery_route: definition.route,
            offered_goals: GOALS
                .into_iter()
                .filter(|goal| route_supports_goal(definition.route, *goal))
                .collect(),
            members: contract
                .members
                .iter()
                .map(|member| member_preview(member, definition.route))
                .collect(),
            max_concurrency: contract.max_concurrency,
            max_outstanding_decisions: contract.max_outstanding_decisions,
            parent_budget_id: &contract.parent_budget_id,
            agent_definition: &contract.agent_definition,
            require_one_writer_per_repository: contract.require_one_writer_per_repository,
            existing,
            can_start: refusals.is_empty(),
            refusals,
        });
    }
    respond(
        200,
        &DefinitionsResponse {
            schema: DEFINITIONS_SCHEMA,
            principal: request.principal,
            definitions: previews,
            non_claims: NON_CLAIMS,
        },
    )
}

#[derive(Serialize)]
struct InboxDecision<'a> {
    identity: &'a ContentId,
    member: &'a str,
    category: &'a DecisionKind,
    state: &'a DecisionRequestState,
    question: &'a str,
    options: &'a [String],
    recommendation: Option<&'a str>,
    work_identity: Option<&'a ContentId>,
    policy_identity: &'a ContentId,
    candidate_identity: Option<&'a ContentId>,
    decision_revision: Option<&'a str>,
    priority: u8,
    created_ms: u64,
    pending_reason: Option<&'a str>,
    resolved_answer: Option<&'a str>,
    active_effort_ms: Option<u64>,
    /// `current`, `changed` or `member_unobserved`.
    scope: &'static str,
    review_roles: &'a [String],
    actions: Vec<&'static str>,
}

#[derive(Serialize)]
struct Inbox<'a> {
    schema: &'static str,
    campaign_id: &'a str,
    store_version: u64,
    campaign_identity: String,
    principal: &'a str,
    admission: AdmissionState,
    operations: Vec<OperatorCommandKind>,
    grant_expires_at_ms: Option<u64>,
    decisions: Vec<InboxDecision<'a>>,
    non_claims: [&'static str; 4],
}

const fn state_rank(state: &DecisionRequestState) -> u8 {
    match state {
        DecisionRequestState::Outstanding => 0,
        DecisionRequestState::Unissued => 1,
        DecisionRequestState::Resolved => 2,
        DecisionRequestState::Expired => 3,
    }
}

fn required_reviews<'a>(state: &'a CampaignState, member: &str) -> &'a [String] {
    state
        .contract
        .members
        .iter()
        .find(|binding| selector_key(&binding.selector) == member)
        .map_or(&[], |binding| binding.required_reviews.as_slice())
}

fn decision_scope(state: &CampaignState, request: &HumanDecisionRequest) -> &'static str {
    if state.decision_in_current_scope(request) {
        "current"
    } else if state.observations.contains_key(&request.member) {
        "changed"
    } else {
        "member_unobserved"
    }
}

/// The operations the caller may submit now; the owner still decides.
fn open_operations(grant: Option<&Grant>, state: &CampaignState) -> Vec<OperatorCommandKind> {
    let Some(grant) = grant else {
        return Vec::new();
    };
    let campaign = &state.contract.campaign_id;
    let outstanding = state
        .decisions
        .values()
        .filter(|request| request.state == DecisionRequestState::Outstanding)
        .count();
    let admission = AdmissionState::of(state);
    OperatorCommandKind::ALL
        .into_iter()
        .filter(|command| grant.permits(campaign, *command))
        .filter(|command| match command {
            OperatorCommandKind::Start => false,
            OperatorCommandKind::Continue => admission == AdmissionState::Paused,
            OperatorCommandKind::Pause => admission == AdmissionState::Open,
            OperatorCommandKind::Cancel => admission != AdmissionState::Cancelled,
            OperatorCommandKind::IssueDecision => {
                admission == AdmissionState::Open
                    && outstanding < usize::from(state.contract.max_outstanding_decisions)
                    && state
                        .decisions
                        .values()
                        .any(|request| request.state == DecisionRequestState::Unissued)
            }
            OperatorCommandKind::ResolveDecision => state
                .decisions
                .values()
                .any(|request| request.state == DecisionRequestState::Outstanding),
        })
        .collect()
}

fn decision_actions(
    grant: Option<&Grant>,
    state: &CampaignState,
    operations: &[OperatorCommandKind],
    request: &HumanDecisionRequest,
    scope: &str,
) -> Vec<&'static str> {
    let Some(grant) = grant else {
        return Vec::new();
    };
    match request.state {
        DecisionRequestState::Outstanding
            if operations.contains(&OperatorCommandKind::ResolveDecision)
                && scope == "current"
                && (request.kind != DecisionKind::RequiredReview
                    || grant.in_audience(required_reviews(state, &request.member))) =>
        {
            vec!["resolve"]
        }
        DecisionRequestState::Unissued
            if operations.contains(&OperatorCommandKind::IssueDecision) =>
        {
            vec!["issue"]
        }
        _ => Vec::new(),
    }
}

// r[impl kiln.implementation.campaign.controls.scenario.decisions]
fn inbox(profile: &Profile, request: &ControlRequest<'_>, campaign_id: &str) -> ControlResponse {
    let Ok(observation) = CampaignObservation::open(&profile.campaign_database) else {
        return failure(503, "campaign_store_unavailable");
    };
    let (state, store_version) = match observation.inspect(campaign_id) {
        Ok(Some(found)) => found,
        Ok(None) => return failure(404, "campaign_not_found"),
        Err(_) => return failure(503, "campaign_store_unavailable"),
    };
    let Ok(campaign_identity) =
        crate::inventory::campaign_projection::identify(&state, store_version)
    else {
        return failure(503, "campaign_controls_byte_bound");
    };
    let grant = (request.role == Role::Operator)
        .then(|| profile.grant(request.principal, request.now_ms))
        .flatten()
        .filter(|grant| grant.campaigns.iter().any(|id| id == campaign_id));
    let operations = open_operations(grant, &state);
    let mut requests: Vec<&HumanDecisionRequest> = state.decisions.values().collect();
    requests.sort_by(|a, b| {
        state_rank(&a.state)
            .cmp(&state_rank(&b.state))
            .then(b.priority.cmp(&a.priority))
            .then(a.created_ms.cmp(&b.created_ms))
            .then(a.identity.as_str().cmp(b.identity.as_str()))
    });
    let decisions = requests
        .into_iter()
        .map(|request| {
            let scope = decision_scope(&state, request);
            InboxDecision {
                identity: &request.identity,
                member: &request.member,
                category: &request.kind,
                state: &request.state,
                question: &request.question,
                options: &request.options,
                recommendation: request.recommendation.as_deref(),
                work_identity: request.work_identity.as_ref(),
                policy_identity: &request.policy_identity,
                candidate_identity: request.candidate_identity.as_ref(),
                decision_revision: request.decision_revision.as_deref(),
                priority: request.priority,
                created_ms: request.created_ms,
                pending_reason: request.pending_reason.as_deref(),
                resolved_answer: request.resolved_answer.as_deref(),
                active_effort_ms: request.active_effort_ms,
                scope,
                review_roles: if request.kind == DecisionKind::RequiredReview {
                    required_reviews(&state, &request.member)
                } else {
                    &[]
                },
                actions: decision_actions(grant, &state, &operations, request, scope),
            }
        })
        .collect();
    respond(
        200,
        &Inbox {
            schema: INBOX_SCHEMA,
            campaign_id,
            store_version,
            campaign_identity,
            principal: request.principal,
            admission: AdmissionState::of(&state),
            grant_expires_at_ms: grant.and_then(|grant| grant.not_after_ms),
            operations,
            decisions,
            non_claims: NON_CLAIMS,
        },
    )
}

/// A typed console intent. Names campaign state by identity and version only.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum CommandBody {
    Start {
        request_id: String,
        definition_identity: ContentId,
    },
    Continue {
        request_id: String,
        campaign_id: String,
        expected_version: u64,
    },
    Pause {
        request_id: String,
        campaign_id: String,
        expected_version: u64,
    },
    Cancel {
        request_id: String,
        campaign_id: String,
        expected_version: u64,
    },
    IssueDecision {
        request_id: String,
        campaign_id: String,
        expected_version: u64,
        decision_identity: ContentId,
    },
    ResolveDecision {
        request_id: String,
        campaign_id: String,
        expected_version: u64,
        decision_identity: ContentId,
        work_identity: Option<ContentId>,
        policy_identity: ContentId,
        candidate_identity: Option<ContentId>,
        answer: String,
    },
}

impl CommandBody {
    fn request_id(&self) -> &str {
        match self {
            Self::Start { request_id, .. }
            | Self::Continue { request_id, .. }
            | Self::Pause { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::IssueDecision { request_id, .. }
            | Self::ResolveDecision { request_id, .. } => request_id,
        }
    }
}

#[derive(Serialize)]
struct CommandResult<'a> {
    schema: &'static str,
    request_id: &'a str,
    replayed: bool,
    actor: &'a str,
    command: OperatorCommandKind,
    campaign_id: &'a str,
    store_version: u64,
    admission: AdmissionState,
    decision: Option<&'a DecisionOutcome>,
    non_claims: [&'static str; 4],
}

fn refused(code: &str) -> ControlResponse {
    match code {
        "campaign_exists" => failure(409, "campaign_exists"),
        "campaign_decision_unknown" => failure(404, "campaign_decision_unknown"),
        "campaign_decision_stale" | "campaign_decision_state" => {
            failure(409, "campaign_decision_stale")
        }
        "campaign_decision_scope_changed" => failure(409, "campaign_decision_scope_changed"),
        "campaign_admission_closed" => failure(409, "campaign_admission_closed"),
        "campaign_decision_queue_full" => failure(409, "campaign_decision_queue_full"),
        _ => failure(409, "campaign_command_refused"),
    }
}

/// Turn an authorized command into an owner intent, or refuse it before the
/// owner is reached.
fn intent(
    profile: &Profile,
    grant: &Grant,
    body: CommandBody,
) -> Result<OperatorIntent, ControlResponse> {
    let authorized = |campaign_id: &str, command: OperatorCommandKind| {
        if grant.permits(campaign_id, command) {
            Ok(())
        } else {
            Err(denied("campaign_command_denied"))
        }
    };
    Ok(match body {
        CommandBody::Start {
            definition_identity,
            ..
        } => {
            let loaded = read_definitions(profile)
                .map_err(|_| failure(503, "campaign_controls_unavailable"))?;
            let definition = loaded
                .into_iter()
                .find(|definition| definition.identity == definition_identity)
                .ok_or_else(|| failure(409, "campaign_definition_changed"))?;
            authorized(&definition.contract.campaign_id, OperatorCommandKind::Start)?;
            if definition.unsupported_goal() {
                return Err(failure(409, "campaign_endpoint_unsupported"));
            }
            OperatorIntent::Start {
                contract: definition.contract,
            }
        }
        CommandBody::Continue {
            campaign_id,
            expected_version,
            ..
        } => {
            authorized(&campaign_id, OperatorCommandKind::Continue)?;
            OperatorIntent::Continue {
                campaign_id,
                expected_version,
            }
        }
        CommandBody::Pause {
            campaign_id,
            expected_version,
            ..
        } => {
            authorized(&campaign_id, OperatorCommandKind::Pause)?;
            OperatorIntent::Pause {
                campaign_id,
                expected_version,
            }
        }
        CommandBody::Cancel {
            campaign_id,
            expected_version,
            ..
        } => {
            authorized(&campaign_id, OperatorCommandKind::Cancel)?;
            OperatorIntent::Cancel {
                campaign_id,
                expected_version,
            }
        }
        CommandBody::IssueDecision {
            campaign_id,
            expected_version,
            decision_identity,
            ..
        } => {
            authorized(&campaign_id, OperatorCommandKind::IssueDecision)?;
            OperatorIntent::IssueDecision {
                campaign_id,
                expected_version,
                identity: decision_identity,
            }
        }
        CommandBody::ResolveDecision {
            campaign_id,
            expected_version,
            decision_identity,
            work_identity,
            policy_identity,
            candidate_identity,
            answer,
            ..
        } => {
            authorized(&campaign_id, OperatorCommandKind::ResolveDecision)?;
            // The decision's kind and member binding cannot change without a
            // new version, which the owner then refuses as stale.
            let observation = CampaignObservation::open(&profile.campaign_database)
                .map_err(|_| failure(503, "campaign_store_unavailable"))?;
            let (state, _) = observation
                .inspect(&campaign_id)
                .map_err(|_| failure(503, "campaign_store_unavailable"))?
                .ok_or_else(|| failure(404, "campaign_not_found"))?;
            let decision = state
                .decisions
                .get(&decision_identity)
                .ok_or_else(|| failure(404, "campaign_decision_unknown"))?;
            if decision.kind == DecisionKind::RequiredReview
                && !grant.in_audience(required_reviews(&state, &decision.member))
            {
                return Err(denied("campaign_command_denied"));
            }
            OperatorIntent::ResolveDecision {
                campaign_id,
                expected_version,
                identity: decision_identity,
                work_identity,
                policy_identity,
                candidate_identity,
                answer,
            }
        }
    })
}

// r[impl kiln.implementation.campaign.controls]
// r[impl kiln.implementation.campaign.controls.scenario.conflict]
fn command(profile: &Profile, request: &ControlRequest<'_>) -> ControlResponse {
    if request.body.len() > MAX_COMMAND_BYTES {
        return failure(400, "campaign_command_invalid");
    }
    let Ok(body) = serde_json::from_slice::<CommandBody>(request.body) else {
        return failure(400, "campaign_command_invalid");
    };
    let request_id = body.request_id().to_owned();
    if !valid_request_id(&request_id) {
        return failure(400, "campaign_command_invalid");
    }
    let Some(grant) = profile.grant(request.principal, request.now_ms) else {
        return denied("campaign_command_denied");
    };
    let intent = match intent(profile, grant, body) {
        Ok(intent) => intent,
        Err(response) => return response,
    };
    let Ok(mut store) = CampaignStore::open(&profile.campaign_database) else {
        return failure(503, "campaign_store_unavailable");
    };
    match store.apply_operator(OperatorCommand {
        request_id,
        actor: request.principal.to_owned(),
        intent,
    }) {
        Ok(AppliedCommand { outcome, replayed }) => respond(
            200,
            &CommandResult {
                schema: RESULT_SCHEMA,
                request_id: &outcome.request_id,
                replayed,
                actor: &outcome.actor,
                command: outcome.command,
                campaign_id: &outcome.campaign_id,
                store_version: outcome.store_version,
                admission: outcome.admission,
                decision: outcome.decision.as_ref(),
                non_claims: NON_CLAIMS,
            },
        ),
        Err(OperatorFailure::Invalid(_)) => failure(400, "campaign_command_invalid"),
        Err(OperatorFailure::Missing) => failure(404, "campaign_not_found"),
        Err(OperatorFailure::Stale { .. }) => failure(409, "campaign_version_conflict"),
        Err(OperatorFailure::RequestConflict) => failure(409, "campaign_request_conflict"),
        Err(OperatorFailure::Refused { code, .. }) => refused(&code),
        Err(OperatorFailure::RetentionExhausted) => {
            failure(503, "campaign_command_retention_exhausted")
        }
        Err(OperatorFailure::Storage(_)) => failure(503, "campaign_store_unavailable"),
    }
}

#[cfg(test)]
#[path = "campaign_controls/tests.rs"]
mod tests;
