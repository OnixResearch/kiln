//! Authenticated GitHub issue label and `/runbook <id>` comment routing to
//! reviewed `#97` runbook dispatch and `#98` fan-out (`OnixResearch/kiln#99`).

use std::collections::{BTreeMap, BTreeSet};

use kiln_implementer::runbook_routing::{
    DeliveryReservation, DeliveryStoreError, DispatchPhase, DispatchReservation, NewAuditEntry,
    NewDispatchReservation, ReplyReservation, RunbookRoutingStore,
};
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};

use crate::comments::GithubCommentPort;
use crate::events::{DeliveryHeaders, EventPolicy, authenticate, valid_delivery, valid_event_name};
use crate::implementation::{ImplementationIssuePolicy, timestamp_millis, valid_label};
use crate::runbook_dispatch::{RunbookDispatchPayload, RunbookDispatchPort, RunbookFanoutPayload};
use crate::{domain, infrastructure};

const MAX_BODY_BYTES: usize = 1_048_576;
const MAX_COMMENT_BODY_BYTES: usize = 262_144;
const MAX_ACTOR_BYTES: usize = 128;
pub const MAX_ROUTING_ACTORS: usize = 256;
pub const MAX_ROUTING_RULES: usize = 128;
pub const MAX_ROUTING_PARAMS: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookRoutingMode {
    #[default]
    Dispatch,
    Fanout,
}

impl RunbookRoutingMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Fanout => "fanout",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingClaimPolicy {
    #[default]
    Refuse,
    Skip,
    Serialize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoutingParamValue {
    Bool(bool),
    Integer(i64),
    Text(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingFanoutOptions {
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub target_globs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_cap: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_policy: Option<RoutingClaimPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelRunbookRoute {
    pub label: String,
    pub runbook_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, RoutingParamValue>,
    #[serde(default)]
    pub mode: RunbookRoutingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<RoutingFanoutOptions>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRunbookRoute {
    pub command: String,
    pub runbook_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, RoutingParamValue>,
    #[serde(default)]
    pub mode: RunbookRoutingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<RoutingFanoutOptions>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookRoutingPolicy {
    pub allowed_actors: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_issue_comments: bool,
    #[serde(default = "default_true")]
    pub allow_pull_request_comments: bool,
    #[serde(default = "default_true")]
    pub require_push_association: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_base_url: Option<String>,
    #[serde(default)]
    pub labels: Vec<LabelRunbookRoute>,
    #[serde(default)]
    pub commands: Vec<CommandRunbookRoute>,
}

impl RunbookRoutingPolicy {
    pub fn validate(&self) -> PortResult<()> {
        if !self.require_push_association {
            return Err(domain(
                "github_runbook_routing_policy",
                "policy.runbook_routing.require_push_association",
                "runbook routing must enforce push association checks",
            ));
        }
        if self.allowed_actors.is_empty() || self.allowed_actors.len() > MAX_ROUTING_ACTORS {
            return Err(domain(
                "github_runbook_routing_policy",
                "policy.runbook_routing.allowed_actors",
                "runbook routing allowed_actors must be non-empty and bounded",
            ));
        }
        let mut seen_actors = BTreeSet::new();
        for actor in &self.allowed_actors {
            if !valid_routing_actor_login(actor) || !seen_actors.insert(actor.as_str()) {
                return Err(domain(
                    "github_runbook_routing_policy",
                    "policy.runbook_routing.allowed_actors",
                    "runbook routing allowed_actors contains an invalid, wildcard, bot, or duplicate actor",
                ));
            }
        }
        if let Some(url) = &self.console_base_url {
            validate_console_base_url(url)?;
        }
        if self.labels.len() > MAX_ROUTING_RULES
            || self.commands.len() > MAX_ROUTING_RULES
            || (self.labels.is_empty() && self.commands.is_empty())
        {
            return Err(domain(
                "github_runbook_routing_policy",
                "policy.runbook_routing",
                "runbook routing must declare bounded label or command mappings",
            ));
        }
        let mut seen_labels = BTreeSet::new();
        for rule in &self.labels {
            if !valid_label(&rule.label) || !seen_labels.insert(rule.label.as_str()) {
                return Err(domain(
                    "github_runbook_routing_policy",
                    "policy.runbook_routing.labels",
                    "runbook routing label rule has an invalid or duplicate label",
                ));
            }
            validate_rule_common(
                &rule.runbook_id,
                rule.target.as_deref(),
                rule.target_ref.as_deref(),
                &rule.params,
                rule.mode,
                rule.fanout.as_ref(),
            )?;
        }
        let mut seen_commands = BTreeSet::new();
        for rule in &self.commands {
            if !valid_runbook_id(&rule.command) || !seen_commands.insert(rule.command.as_str()) {
                return Err(domain(
                    "github_runbook_routing_policy",
                    "policy.runbook_routing.commands",
                    "runbook routing command rule has an invalid or duplicate command",
                ));
            }
            validate_rule_common(
                &rule.runbook_id,
                rule.target.as_deref(),
                rule.target_ref.as_deref(),
                &rule.params,
                rule.mode,
                rule.fanout.as_ref(),
            )?;
        }
        Ok(())
    }

    #[must_use]
    pub fn allows_actor(&self, login: &str) -> bool {
        self.allowed_actors.iter().any(|actor| actor == login)
    }
}

fn validate_rule_common(
    runbook_id: &str,
    target: Option<&str>,
    target_ref: Option<&str>,
    params: &BTreeMap<String, RoutingParamValue>,
    mode: RunbookRoutingMode,
    fanout: Option<&RoutingFanoutOptions>,
) -> PortResult<()> {
    if !valid_runbook_id(runbook_id) {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.runbook_id",
            "runbook routing rule declares an invalid runbook_id",
        ));
    }
    if target.is_some() && target_ref.is_some() {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.target",
            "runbook routing rule cannot specify both target and target_ref",
        ));
    }
    if let Some(target) = target
        && !valid_target_spec(target)
    {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.target",
            "runbook routing rule declares an invalid target",
        ));
    }
    if let Some(reference) = target_ref
        && !valid_target_ref(reference)
    {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.target_ref",
            "runbook routing rule declares an invalid target_ref",
        ));
    }
    if params.len() > MAX_ROUTING_PARAMS {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.params",
            "runbook routing rule exceeds parameter bound",
        ));
    }
    for (key, value) in params {
        if !valid_param_name(key) {
            return Err(domain(
                "github_runbook_routing_policy",
                "policy.runbook_routing.params",
                "runbook routing parameter name is invalid",
            ));
        }
        if let RoutingParamValue::Text(text) = value
            && (text.len() > 256 || text.chars().any(char::is_control))
        {
            return Err(domain(
                "github_runbook_routing_policy",
                "policy.runbook_routing.params",
                "runbook routing text parameter is invalid or exceeds 256 bytes",
            ));
        }
    }
    match mode {
        RunbookRoutingMode::Dispatch => {
            if fanout.is_some() || (target.is_none() && target_ref.is_none()) {
                return Err(domain(
                    "github_runbook_routing_policy",
                    "policy.runbook_routing.mode",
                    "dispatch rule requires target or target_ref and forbids fanout options",
                ));
            }
        }
        RunbookRoutingMode::Fanout => {
            let has_fanout_targets = fanout
                .is_some_and(|opts| !opts.targets.is_empty() || !opts.target_globs.is_empty());
            if target.is_none() && target_ref.is_none() && !has_fanout_targets {
                return Err(domain(
                    "github_runbook_routing_policy",
                    "policy.runbook_routing.fanout",
                    "fanout rule requires at least one target, target_ref, or target_glob",
                ));
            }
            if let Some(opts) = fanout {
                for t in &opts.targets {
                    if !valid_target_spec(t) {
                        return Err(domain(
                            "github_runbook_routing_policy",
                            "policy.runbook_routing.fanout.targets",
                            "fanout target is invalid",
                        ));
                    }
                }
                for glob in &opts.target_globs {
                    if glob.is_empty()
                        || glob.len() > 512
                        || glob.contains("..")
                        || glob.chars().any(|ch| ch.is_whitespace() || ch.is_control())
                    {
                        return Err(domain(
                            "github_runbook_routing_policy",
                            "policy.runbook_routing.fanout.target_globs",
                            "fanout target_glob is invalid",
                        ));
                    }
                }
                if opts.max_concurrency.is_some_and(|c| c == 0 || c > 32)
                    || opts.budget_cap.is_some_and(|b| b == 0 || b > 10_000)
                {
                    return Err(domain(
                        "github_runbook_routing_policy",
                        "policy.runbook_routing.fanout",
                        "fanout concurrency or budget_cap is outside bounds",
                    ));
                }
                if let Some(scope) = &opts.scope_path
                    && !valid_scope_path(scope)
                {
                    return Err(domain(
                        "github_runbook_routing_policy",
                        "policy.runbook_routing.fanout.scope_path",
                        "fanout scope_path is invalid",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn valid_runbook_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

pub(crate) fn valid_routing_actor_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ACTOR_BYTES
        && !value.contains('*')
        && !value.to_ascii_lowercase().ends_with("[bot]")
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

fn valid_param_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn valid_target_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.starts_with('-')
        && !value.contains("..")
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

fn valid_target_spec(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 1280
        || value.contains("..")
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return false;
    }
    let Some((repo, reference)) = value.split_once('@') else {
        return false;
    };
    if reference.contains('@') || !valid_target_ref(reference) {
        return false;
    }
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };
    !owner.is_empty()
        && !name.is_empty()
        && !name.contains('/')
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

fn valid_scope_path(value: &str) -> bool {
    if value == "." {
        return true;
    }
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !value.contains('\\')
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("//")
        && !value.split('/').any(|seg| seg == "." || seg == "..")
}

pub fn validate_console_base_url(value: &str) -> PortResult<()> {
    let Some(rest) = value.strip_prefix("https://") else {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.console_base_url",
            "console_base_url must use https://",
        ));
    };
    if value.len() > 512
        || value.ends_with('/')
        || rest.is_empty()
        || rest.starts_with('/')
        || rest.split('/').any(str::is_empty)
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        || value.contains(['@', '?', '#'])
    {
        return Err(domain(
            "github_runbook_routing_policy",
            "policy.runbook_routing.console_base_url",
            "console_base_url is invalid",
        ));
    }
    Ok(())
}

/// Strictly parse an exact `/runbook <id>` command comment.
///
/// After `.trim()`, the entire comment body must equal `/runbook <id>`, with
/// ASCII space or tab separation only, a valid `RunbookId` (`^[a-z][a-z0-9-]{0,63}$`),
/// and zero trailing tokens, flags, targets, or extra lines.
#[must_use]
pub fn parse_runbook_comment_command(body: &str) -> Option<&str> {
    let trimmed = body.trim();
    let rest = trimmed.strip_prefix("/runbook")?;
    let first_sep = rest.bytes().next()?;
    if !matches!(first_sep, b' ' | b'\t') {
        return None;
    }
    let id = rest.trim_start_matches([' ', '\t']);
    if valid_runbook_id(id) { Some(id) } else { None }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestHeadRepo {
    pub id: u64,
    pub name: String,
    pub full_name: Option<String>,
    pub owner_login: String,
    pub fork: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestHead {
    pub repo: Option<PullRequestHeadRepo>,
}

pub trait PullRequestOriginCheck {
    fn check_pull_request_head(
        &mut self,
        owner: &str,
        repository: &str,
        pull_request_number: u64,
    ) -> PortResult<PullRequestHead>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingTriggerKind {
    Label,
    Comment,
}

impl RoutingTriggerKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::Comment => "comment",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingAuditRecord {
    pub recorded_at_ms: u64,
    pub repository_id: u64,
    pub repository: String,
    pub actor: String,
    pub trigger_kind: &'static str,
    pub issue_number: u64,
    pub comment_id: Option<u64>,
    pub label_name: Option<String>,
    pub runbook_id: Option<String>,
    pub outcome: &'static str,
    pub decision_code: String,
    pub request_id: Option<String>,
    pub dispatch_id: Option<String>,
}

impl RoutingAuditRecord {
    fn to_new_audit_entry(&self) -> NewAuditEntry {
        NewAuditEntry {
            recorded_at_ms: self.recorded_at_ms,
            repository_id: self.repository_id,
            repository: self.repository.clone(),
            actor: self.actor.clone(),
            trigger_kind: self.trigger_kind.to_string(),
            issue_number: self.issue_number,
            comment_id: self.comment_id,
            label_name: self.label_name.clone(),
            runbook_id: self.runbook_id.clone(),
            outcome: self.outcome.to_string(),
            decision_code: self.decision_code.clone(),
            request_id: self.request_id.clone(),
            dispatch_id: self.dispatch_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingMatch {
    pub source_key: String,
    pub request_id: String,
    pub trigger_kind: RoutingTriggerKind,
    pub repository_id: u64,
    pub repository_key: String,
    pub issue_number: u64,
    pub comment_id: Option<u64>,
    pub label_name: Option<String>,
    pub actor: String,
    pub runbook_id: String,
    pub mode: RunbookRoutingMode,
    pub target: Option<String>,
    pub params: BTreeMap<String, RoutingParamValue>,
    pub fanout: Option<RoutingFanoutOptions>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingRefusal {
    pub code: &'static str,
    pub field: &'static str,
    pub message: &'static str,
    pub audit: RoutingAuditRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutingOutcome {
    Matched(RoutingMatch),
    Ignored {
        reason: &'static str,
        audit: Option<RoutingAuditRecord>,
    },
    Refused(RoutingRefusal),
}

#[derive(Deserialize)]
struct WireActor {
    login: String,
    #[serde(default, rename = "type")]
    actor_type: Option<String>,
}

#[derive(Deserialize)]
struct WireRepository {
    id: u64,
    name: String,
    #[serde(default)]
    full_name: Option<String>,
    owner: WireActor,
}

#[derive(Deserialize)]
struct WireLabel {
    name: String,
}

#[derive(Deserialize)]
struct WireIssue {
    number: u64,
    state: String,
    updated_at: String,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
    #[serde(default)]
    author_association: Option<String>,
}

#[derive(Deserialize)]
struct WireComment {
    id: u64,
    body: String,
    user: WireActor,
    #[serde(default)]
    author_association: Option<String>,
}

#[derive(Deserialize)]
struct IssuesRoutingEnvelope {
    action: String,
    issue: WireIssue,
    repository: WireRepository,
    sender: WireActor,
    #[serde(default)]
    label: Option<WireLabel>,
}

#[derive(Deserialize)]
struct IssueCommentRoutingEnvelope {
    action: String,
    issue: WireIssue,
    comment: WireComment,
    repository: WireRepository,
    sender: WireActor,
}

#[must_use]
pub fn comment_source_key(repo_id: u64, comment_id: u64, runbook_id: &str) -> String {
    format!("comment:{repo_id}:{comment_id}:{runbook_id}")
}

#[must_use]
pub fn label_source_key(repo_id: u64, issue_number: u64, label_name: &str) -> String {
    let label_hash = blake3::hash(label_name.as_bytes()).to_hex();
    format!("label:{repo_id}:{issue_number}:{label_hash}")
}

pub fn ensure_repository_enrolled(
    policy: &ImplementationIssuePolicy,
    enrollment: &kiln_core::implementation::Enrollment,
) -> PortResult<()> {
    if !enrollment.repositories.contains(&policy.repository_key()) {
        return Err(domain(
            "implementation_unenrolled",
            "work.enrollment",
            "repository or actor is not enrolled for implementation",
        ));
    }
    Ok(())
}

#[must_use]
pub fn deterministic_request_id(
    source_key: &str,
    runbook_id: &str,
    mode: RunbookRoutingMode,
    target: Option<&str>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in [
        "kiln.github.runbook-route.v1",
        source_key,
        runbook_id,
        mode.as_str(),
        target.unwrap_or(""),
    ] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("gh-rb-{}", hasher.finalize().to_hex())
}

#[must_use]
pub fn reply_marker(request_id: &str) -> String {
    format!("<!-- kiln-runbook-dispatch:{request_id} -->")
}

/// Format a concise GitHub issue/PR status reply from validated bounded
/// identifiers only. Never echoes credentials, tokens, params, paths, or logs.
pub fn format_runbook_reply_comment(
    console_base_url: Option<&str>,
    runbook_id: &str,
    mode: RunbookRoutingMode,
    dispatch_id: &str,
    target: Option<&str>,
    source_revision: Option<&str>,
    request_id: &str,
) -> PortResult<String> {
    if !valid_runbook_id(runbook_id)
        || !valid_safe_id(dispatch_id)
        || !valid_safe_id(request_id)
        || target.is_some_and(|t| !valid_target_spec(t))
        || source_revision.is_some_and(|rev| !valid_safe_id(rev))
    {
        return Err(domain(
            "github_comment_reply_invalid",
            "reply",
            "runbook reply contains invalid identifiers",
        ));
    }
    if let Some(base) = console_base_url {
        validate_console_base_url(base)?;
    }
    let marker = reply_marker(request_id);
    let body = match mode {
        RunbookRoutingMode::Dispatch => {
            let detail = match (target, source_revision) {
                (Some(t), Some(rev)) => format!(" (target `{t}`, revision `{rev}`)"),
                (Some(t), None) => format!(" (target `{t}`)"),
                (None, Some(rev)) => format!(" (revision `{rev}`)"),
                (None, None) => String::new(),
            };
            match console_base_url {
                Some(base) => format!(
                    "Kiln dispatched runbook `{runbook_id}` as run `{dispatch_id}`{detail}: {base}/?page=runs&run={dispatch_id}&view=pipeline\n{marker}"
                ),
                None => format!(
                    "Kiln dispatched runbook `{runbook_id}` as run `{dispatch_id}`{detail}.\n{marker}"
                ),
            }
        }
        RunbookRoutingMode::Fanout => match console_base_url {
            Some(base) => format!(
                "Kiln dispatched runbook `{runbook_id}` as fan-out `{dispatch_id}`: {base}/?page=runbooks&runbook={runbook_id}&fanout={dispatch_id}\n{marker}"
            ),
            None => format!(
                "Kiln dispatched runbook `{runbook_id}` as fan-out `{dispatch_id}`.\n{marker}"
            ),
        },
    };
    Ok(body)
}

fn valid_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Admit and route one authenticated GitHub webhook delivery (`issues` or
/// `issue_comment`) against the reviewed `ImplementationIssuePolicy`.
pub fn admit_runbook_routing_event<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    enrolled_actors: &BTreeSet<String>,
    headers: &DeliveryHeaders,
    body: &[u8],
    secret: &[u8],
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    policy.validate()?;
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err(domain(
            "github_body_bound",
            "body",
            "GitHub event body is outside its admitted bound",
        ));
    }
    if !valid_delivery(&headers.delivery) || !valid_event_name(&headers.event) {
        return Err(domain(
            "github_event_headers",
            "headers",
            "GitHub delivery or event header is invalid",
        ));
    }
    authenticate(&headers.signature, body, secret)?;
    admit_runbook_routing_event_verified(
        policy,
        enrolled_actors,
        &headers.event,
        body,
        pr_check,
        now_ms,
    )
}

fn admit_runbook_routing_event_verified<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    enrolled_actors: &BTreeSet<String>,
    event: &str,
    body: &[u8],
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    let Some(routing) = &policy.runbook_routing else {
        let _: serde::de::IgnoredAny = serde_json::from_slice(body).map_err(|_| {
            domain(
                "github_event_payload",
                "body",
                "GitHub event payload is malformed or inconsistent",
            )
        })?;
        return Ok(RoutingOutcome::Ignored {
            reason: "runbook_routing_disabled",
            audit: None,
        });
    };
    match event {
        "issues" => admit_issues_event(policy, routing, enrolled_actors, body, pr_check, now_ms),
        "issue_comment" => {
            admit_issue_comment_event(policy, routing, enrolled_actors, body, pr_check, now_ms)
        }
        _ => {
            let _: serde::de::IgnoredAny = serde_json::from_slice(body).map_err(|_| {
                domain(
                    "github_event_payload",
                    "body",
                    "GitHub event payload is malformed or inconsistent",
                )
            })?;
            Ok(RoutingOutcome::Ignored {
                reason: "unsupported_event",
                audit: None,
            })
        }
    }
}

fn admit_issues_event<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    routing: &RunbookRoutingPolicy,
    enrolled_actors: &BTreeSet<String>,
    body: &[u8],
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    let envelope: IssuesRoutingEnvelope = serde_json::from_slice(body).map_err(|_| {
        domain(
            "github_event_payload",
            "body",
            "GitHub issue payload is malformed or inconsistent",
        )
    })?;
    if !repository_matches_policy(&policy.repository, &envelope.repository) {
        return Ok(refusal(
            "github_repository_mismatch",
            "repository",
            "GitHub event repository does not match the admitted policy",
            RoutingAuditRecord {
                recorded_at_ms: now_ms,
                repository_id: envelope.repository.id,
                repository: format_repo(&envelope.repository),
                actor: sanitize_actor(&envelope.sender.login),
                trigger_kind: "label",
                issue_number: envelope.issue.number,
                comment_id: None,
                label_name: envelope.label.as_ref().map(|l| l.name.clone()),
                runbook_id: None,
                outcome: "refused",
                decision_code: "github_repository_mismatch".to_string(),
                request_id: None,
                dispatch_id: None,
            },
        ));
    }
    if envelope.issue.number == 0 || !matches!(envelope.issue.state.as_str(), "open" | "closed") {
        return Err(domain(
            "github_issue_payload",
            "issue",
            "GitHub issue payload is malformed or inconsistent",
        ));
    }
    let _updated_at_ms = timestamp_millis(&envelope.issue.updated_at).ok_or_else(|| {
        domain(
            "github_issue_payload",
            "issue.updated_at",
            "GitHub issue timestamp is invalid",
        )
    })?;
    if envelope.action != "labeled" || envelope.issue.state != "open" {
        return Ok(RoutingOutcome::Ignored {
            reason: if envelope.action != "labeled" {
                "issue_action_ignored"
            } else {
                "issue_not_open"
            },
            audit: None,
        });
    }
    let Some(label) = envelope.label else {
        return Ok(RoutingOutcome::Ignored {
            reason: "missing_label",
            audit: None,
        });
    };
    if !valid_label(&label.name) {
        return Err(domain(
            "github_issue_payload",
            "label",
            "GitHub issue label is invalid",
        ));
    }
    let Some(route) = routing.labels.iter().find(|r| r.label == label.name) else {
        return Ok(RoutingOutcome::Ignored {
            reason: "unmapped_label",
            audit: None,
        });
    };
    evaluate_label_route(
        policy,
        routing,
        enrolled_actors,
        route,
        &envelope.sender,
        envelope.issue.number,
        envelope.issue.author_association.as_deref(),
        envelope.issue.pull_request.is_some(),
        pr_check,
        now_ms,
    )
}

fn admit_issue_comment_event<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    routing: &RunbookRoutingPolicy,
    enrolled_actors: &BTreeSet<String>,
    body: &[u8],
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    let envelope: IssueCommentRoutingEnvelope = serde_json::from_slice(body).map_err(|_| {
        domain(
            "github_event_payload",
            "body",
            "GitHub issue_comment payload is malformed or inconsistent",
        )
    })?;
    if !repository_matches_policy(&policy.repository, &envelope.repository) {
        return Ok(refusal(
            "github_repository_mismatch",
            "repository",
            "GitHub event repository does not match the admitted policy",
            RoutingAuditRecord {
                recorded_at_ms: now_ms,
                repository_id: envelope.repository.id,
                repository: format_repo(&envelope.repository),
                actor: sanitize_actor(&envelope.sender.login),
                trigger_kind: "comment",
                issue_number: envelope.issue.number,
                comment_id: Some(envelope.comment.id),
                label_name: None,
                runbook_id: None,
                outcome: "refused",
                decision_code: "github_repository_mismatch".to_string(),
                request_id: None,
                dispatch_id: None,
            },
        ));
    }
    if envelope.issue.number == 0
        || envelope.comment.id == 0
        || envelope.comment.body.len() > MAX_COMMENT_BODY_BYTES
        || !matches!(envelope.issue.state.as_str(), "open" | "closed")
    {
        return Err(domain(
            "github_issue_comment_payload",
            "issue_comment",
            "GitHub issue_comment payload is malformed or inconsistent",
        ));
    }
    if envelope.action != "created" {
        return Ok(RoutingOutcome::Ignored {
            reason: "comment_action_ignored",
            audit: None,
        });
    }
    let Some(command_id) = parse_runbook_comment_command(&envelope.comment.body) else {
        return Ok(RoutingOutcome::Ignored {
            reason: "not_runbook_command",
            audit: None,
        });
    };
    let Some(route) = routing.commands.iter().find(|r| r.command == command_id) else {
        return Ok(RoutingOutcome::Ignored {
            reason: "unmapped_command",
            audit: None,
        });
    };
    if !is_admitted_actor(&envelope.sender, enrolled_actors, routing)
        || envelope.sender.login != envelope.comment.user.login
    {
        return Ok(refusal(
            "github_actor_denied",
            "sender.login",
            "comment sender is not allowlisted or does not match comment author",
            RoutingAuditRecord {
                recorded_at_ms: now_ms,
                repository_id: policy.repository.repository_id,
                repository: policy.repository_key(),
                actor: sanitize_actor(&envelope.sender.login),
                trigger_kind: "comment",
                issue_number: envelope.issue.number,
                comment_id: Some(envelope.comment.id),
                label_name: None,
                runbook_id: Some(route.runbook_id.clone()),
                outcome: "refused",
                decision_code: "github_actor_denied".to_string(),
                request_id: None,
                dispatch_id: None,
            },
        ));
    }
    evaluate_comment_route(
        policy,
        routing,
        enrolled_actors,
        route,
        &envelope.comment.user,
        envelope.issue.number,
        envelope.comment.id,
        envelope.comment.author_association.as_deref(),
        envelope.issue.pull_request.is_some(),
        pr_check,
        now_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_label_route<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    routing: &RunbookRoutingPolicy,
    enrolled_actors: &BTreeSet<String>,
    route: &LabelRunbookRoute,
    actor: &WireActor,
    issue_number: u64,
    author_association: Option<&str>,
    is_pull_request: bool,
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    let repository_key = policy.repository_key();
    let repo_id = policy.repository.repository_id;
    let base_audit = |code: &str| RoutingAuditRecord {
        recorded_at_ms: now_ms,
        repository_id: repo_id,
        repository: repository_key.clone(),
        actor: sanitize_actor(&actor.login),
        trigger_kind: "label",
        issue_number,
        comment_id: None,
        label_name: Some(route.label.clone()),
        runbook_id: Some(route.runbook_id.clone()),
        outcome: "refused",
        decision_code: code.to_string(),
        request_id: None,
        dispatch_id: None,
    };
    if !is_admitted_actor(actor, enrolled_actors, routing) {
        return Ok(refusal(
            "github_actor_denied",
            "sender.login",
            "GitHub actor is not allowlisted for runbook routing",
            base_audit("github_actor_denied"),
        ));
    }
    if routing.require_push_association
        && !kiln_core::untrusted::author_has_push_access(author_association)
    {
        return Ok(refusal(
            "github_outside_contributor_denied",
            "issue.author_association",
            "outside contributors cannot trigger runbook routing",
            base_audit("github_outside_contributor_denied"),
        ));
    }
    if is_pull_request
        && let Some(refusal_outcome) = verify_pr_origin(policy, pr_check, issue_number, base_audit)?
    {
        return Ok(refusal_outcome);
    }
    let resolved_target =
        resolve_rule_target(policy, route.target.as_deref(), route.target_ref.as_deref());
    let source_key = label_source_key(repo_id, issue_number, &route.label);
    let request_id = deterministic_request_id(
        &source_key,
        &route.runbook_id,
        route.mode,
        resolved_target.as_deref(),
    );
    Ok(RoutingOutcome::Matched(RoutingMatch {
        source_key,
        request_id,
        trigger_kind: RoutingTriggerKind::Label,
        repository_id: repo_id,
        repository_key,
        issue_number,
        comment_id: None,
        label_name: Some(route.label.clone()),
        actor: actor.login.clone(),
        runbook_id: route.runbook_id.clone(),
        mode: route.mode,
        target: resolved_target,
        params: route.params.clone(),
        fanout: route.fanout.clone(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_comment_route<C: PullRequestOriginCheck>(
    policy: &ImplementationIssuePolicy,
    routing: &RunbookRoutingPolicy,
    enrolled_actors: &BTreeSet<String>,
    route: &CommandRunbookRoute,
    actor: &WireActor,
    issue_number: u64,
    comment_id: u64,
    author_association: Option<&str>,
    is_pull_request: bool,
    pr_check: &mut C,
    now_ms: u64,
) -> PortResult<RoutingOutcome> {
    let repository_key = policy.repository_key();
    let repo_id = policy.repository.repository_id;
    let base_audit = |code: &str| RoutingAuditRecord {
        recorded_at_ms: now_ms,
        repository_id: repo_id,
        repository: repository_key.clone(),
        actor: sanitize_actor(&actor.login),
        trigger_kind: "comment",
        issue_number,
        comment_id: Some(comment_id),
        label_name: None,
        runbook_id: Some(route.runbook_id.clone()),
        outcome: "refused",
        decision_code: code.to_string(),
        request_id: None,
        dispatch_id: None,
    };
    if !is_pull_request && !routing.allow_issue_comments {
        return Ok(refusal(
            "github_issue_comment_denied",
            "issue_comment",
            "issue comments are disabled for runbook routing",
            base_audit("github_issue_comment_denied"),
        ));
    }
    if is_pull_request && !routing.allow_pull_request_comments {
        return Ok(refusal(
            "github_pull_request_comment_denied",
            "issue_comment",
            "pull request comments are disabled for runbook routing",
            base_audit("github_pull_request_comment_denied"),
        ));
    }
    if !is_admitted_actor(actor, enrolled_actors, routing) {
        return Ok(refusal(
            "github_actor_denied",
            "comment.user.login",
            "GitHub actor is not allowlisted for runbook routing",
            base_audit("github_actor_denied"),
        ));
    }
    if routing.require_push_association
        && !kiln_core::untrusted::author_has_push_access(author_association)
    {
        return Ok(refusal(
            "github_outside_contributor_denied",
            "comment.author_association",
            "outside contributors cannot trigger runbook routing",
            base_audit("github_outside_contributor_denied"),
        ));
    }
    if is_pull_request
        && let Some(refusal_outcome) = verify_pr_origin(policy, pr_check, issue_number, base_audit)?
    {
        return Ok(refusal_outcome);
    }
    let resolved_target =
        resolve_rule_target(policy, route.target.as_deref(), route.target_ref.as_deref());
    let source_key = comment_source_key(repo_id, comment_id, &route.runbook_id);
    let request_id = deterministic_request_id(
        &source_key,
        &route.runbook_id,
        route.mode,
        resolved_target.as_deref(),
    );
    Ok(RoutingOutcome::Matched(RoutingMatch {
        source_key,
        request_id,
        trigger_kind: RoutingTriggerKind::Comment,
        repository_id: repo_id,
        repository_key,
        issue_number,
        comment_id: Some(comment_id),
        label_name: None,
        actor: actor.login.clone(),
        runbook_id: route.runbook_id.clone(),
        mode: route.mode,
        target: resolved_target,
        params: route.params.clone(),
        fanout: route.fanout.clone(),
    }))
}

fn verify_pr_origin<C: PullRequestOriginCheck, F: Fn(&str) -> RoutingAuditRecord>(
    policy: &ImplementationIssuePolicy,
    pr_check: &mut C,
    pull_request_number: u64,
    make_audit: F,
) -> PortResult<Option<RoutingOutcome>> {
    let head = match pr_check.check_pull_request_head(
        &policy.repository.owner,
        &policy.repository.repository,
        pull_request_number,
    ) {
        Ok(head) => head,
        Err(_) => {
            return Ok(Some(refusal(
                "github_pr_origin_unverified",
                "pull_request.head.repo",
                "pull request head repository could not be verified",
                make_audit("github_pr_origin_unverified"),
            )));
        }
    };
    let Some(repo) = head.repo else {
        return Ok(Some(refusal(
            "github_fork_denied",
            "pull_request.head.repo",
            "pull request head repository is missing or a deleted fork",
            make_audit("github_fork_denied"),
        )));
    };
    let full_name_ok = repo.full_name.as_deref().is_none_or(|full| {
        full.split_once('/').is_some_and(|(o, n)| {
            o.eq_ignore_ascii_case(&policy.repository.owner)
                && n.eq_ignore_ascii_case(&policy.repository.repository)
        })
    });
    let same_repo = !repo.fork
        && repo.id == policy.repository.repository_id
        && repo
            .owner_login
            .eq_ignore_ascii_case(&policy.repository.owner)
        && repo
            .name
            .eq_ignore_ascii_case(&policy.repository.repository)
        && full_name_ok;
    if !same_repo {
        return Ok(Some(refusal(
            "github_fork_denied",
            "pull_request.head.repo",
            "fork pull requests are refused for runbook routing",
            make_audit("github_fork_denied"),
        )));
    }
    Ok(None)
}

fn resolve_rule_target(
    policy: &ImplementationIssuePolicy,
    target: Option<&str>,
    target_ref: Option<&str>,
) -> Option<String> {
    if let Some(target) = target {
        return Some(target.to_string());
    }
    target_ref.map(|reference| {
        format!(
            "{}/{}@{reference}",
            policy.repository.owner, policy.repository.repository
        )
    })
}

fn is_admitted_actor(
    actor: &WireActor,
    enrolled_actors: &BTreeSet<String>,
    routing: &RunbookRoutingPolicy,
) -> bool {
    if actor
        .actor_type
        .as_deref()
        .is_some_and(|kind| !kind.eq_ignore_ascii_case("User"))
    {
        return false;
    }
    valid_routing_actor_login(&actor.login)
        && enrolled_actors.contains(&actor.login)
        && routing.allows_actor(&actor.login)
}

fn repository_matches_policy(policy: &EventPolicy, repo: &WireRepository) -> bool {
    if repo.id == 0
        || repo.id != policy.repository_id
        || !repo.owner.login.eq_ignore_ascii_case(&policy.owner)
        || !repo.name.eq_ignore_ascii_case(&policy.repository)
    {
        return false;
    }
    if let Some(full_name) = &repo.full_name {
        let Some((owner, name)) = full_name.split_once('/') else {
            return false;
        };
        if !owner.eq_ignore_ascii_case(&repo.owner.login) || !name.eq_ignore_ascii_case(&repo.name)
        {
            return false;
        }
    }
    true
}

fn format_repo(repo: &WireRepository) -> String {
    format!(
        "{}/{}",
        sanitize_actor(&repo.owner.login),
        sanitize_actor(&repo.name)
    )
}

fn sanitize_actor(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_ACTOR_BYTES)
        .collect()
}

fn refusal(
    code: &'static str,
    field: &'static str,
    message: &'static str,
    audit: RoutingAuditRecord,
) -> RoutingOutcome {
    RoutingOutcome::Refused(RoutingRefusal {
        code,
        field,
        message,
        audit,
    })
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RunbookRoutingSummary {
    pub drained: usize,
    pub matched: usize,
    pub dispatched: usize,
    pub replayed: usize,
    pub ignored: usize,
    pub refused: usize,
    pub dispatch_errors: usize,
    pub comments_posted: usize,
    pub comment_errors: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RunbookPollSummary {
    pub polled_comments: usize,
    pub polled_issues: usize,
    pub matched: usize,
    pub dispatched: usize,
    pub replayed: usize,
    pub ignored: usize,
    pub refused: usize,
    pub dispatch_errors: usize,
    pub comments_posted: usize,
    pub comment_errors: usize,
}

/// Drain up to `limit` queued GitHub deliveries through runbook routing,
/// dispatch admitted routes to `kiln-web-service`, and post safe status reply
/// comments.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn drain_runbooks_queue<C, D>(
    queue_config: &crate::profile::ImplementationQueueConfig,
    secret: &[u8],
    policy: &ImplementationIssuePolicy,
    enrollment: &kiln_core::implementation::Enrollment,
    store: &mut RunbookRoutingStore,
    comments: &mut C,
    dispatcher: &mut D,
    limit: usize,
    now_ms: u64,
) -> PortResult<RunbookRoutingSummary>
where
    C: GithubCommentPort,
    D: RunbookDispatchPort,
{
    policy.validate()?;
    ensure_repository_enrolled(policy, enrollment)?;
    if secret.is_empty() || secret.len() > 65_536 {
        return Err(domain(
            "github_authentication",
            "signature",
            "GitHub event authentication failed",
        ));
    }
    let _routing_lock = acquire_routing_lock(store)?;
    let mut queue =
        kiln_implementer::DeliveryQueue::open(&queue_config.path, queue_config.maximum_deliveries)
            .map_err(|_| {
                infrastructure(
                    "github_implementation_queue",
                    "implementation delivery queue is unavailable",
                )
            })?;
    let deliveries = queue.list(limit).map_err(|_| {
        infrastructure(
            "github_implementation_queue",
            "implementation delivery queue could not be read",
        )
    })?;
    let console_base_url = policy
        .runbook_routing
        .as_ref()
        .and_then(|r| r.console_base_url.as_deref());
    let mut summary = RunbookRoutingSummary::default();
    for delivery in deliveries {
        let headers = DeliveryHeaders {
            delivery: delivery.delivery_id.clone(),
            event: delivery.event.clone(),
            signature: delivery.signature.clone(),
        };
        let trigger_kind = match headers.event.as_str() {
            "issues" => "label",
            "issue_comment" => "comment",
            _ => "unknown",
        };
        let precheck = (|| -> PortResult<()> {
            if delivery.body.is_empty() || delivery.body.len() > MAX_BODY_BYTES {
                return Err(domain(
                    "github_body_bound",
                    "body",
                    "GitHub event body is outside its admitted bound",
                ));
            }
            if !valid_delivery(&headers.delivery) || !valid_event_name(&headers.event) {
                return Err(domain(
                    "github_event_headers",
                    "headers",
                    "GitHub delivery or event header is invalid",
                ));
            }
            authenticate(&headers.signature, &delivery.body, secret)?;
            Ok(())
        })();
        if let Err(error) = precheck {
            match error {
                PortFailure::Domain(diag) => {
                    store
                        .record_audit(&NewAuditEntry {
                            recorded_at_ms: now_ms,
                            repository_id: policy.repository.repository_id,
                            repository: policy.repository_key(),
                            actor: "unknown".to_string(),
                            trigger_kind: trigger_kind.to_string(),
                            issue_number: 0,
                            comment_id: None,
                            label_name: None,
                            runbook_id: None,
                            outcome: "refused".to_string(),
                            decision_code: diag.code,
                            request_id: None,
                            dispatch_id: None,
                        })
                        .map_err(|e| infrastructure("github_routing_store", &e))?;
                    queue.remove(&delivery.delivery_id).map_err(|_| {
                        infrastructure(
                            "github_implementation_queue",
                            "refused delivery could not be removed from the queue",
                        )
                    })?;
                    summary.refused += 1;
                    summary.drained += 1;
                    continue;
                }
                other => return Err(other),
            }
        }

        let body_identity = blake3::hash(&delivery.body).to_hex().to_string();
        match store.reserve_delivery(&headers.delivery, &body_identity, &headers.event, now_ms) {
            Ok(DeliveryReservation::Fresh) => {}
            Ok(DeliveryReservation::Duplicate { outcome: prev }) => {
                if prev != "pending" {
                    summary.ignored += 1;
                    queue.remove(&delivery.delivery_id).map_err(|_| {
                        infrastructure(
                            "github_implementation_queue",
                            "duplicate delivery could not be removed from the queue",
                        )
                    })?;
                    summary.drained += 1;
                    continue;
                }
            }
            Err(DeliveryStoreError::Conflict) => {
                store
                    .record_audit(&NewAuditEntry {
                        recorded_at_ms: now_ms,
                        repository_id: policy.repository.repository_id,
                        repository: policy.repository_key(),
                        actor: "unknown".to_string(),
                        trigger_kind: trigger_kind.to_string(),
                        issue_number: 0,
                        comment_id: None,
                        label_name: None,
                        runbook_id: None,
                        outcome: "refused".to_string(),
                        decision_code: "github_delivery_conflict".to_string(),
                        request_id: None,
                        dispatch_id: None,
                    })
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                queue.remove(&delivery.delivery_id).map_err(|_| {
                    infrastructure(
                        "github_implementation_queue",
                        "conflicting delivery could not be removed from the queue",
                    )
                })?;
                summary.refused += 1;
                summary.drained += 1;
                continue;
            }
            Err(DeliveryStoreError::Capacity) => {
                return Err(infrastructure(
                    "github_routing_capacity",
                    "runbook routing delivery ledger is full",
                ));
            }
            Err(DeliveryStoreError::Invalid(msg) | DeliveryStoreError::Storage(msg)) => {
                return Err(infrastructure("github_routing_store", &msg));
            }
        }

        let outcome = match admit_runbook_routing_event_verified(
            policy,
            &enrollment.actors,
            &headers.event,
            &delivery.body,
            comments,
            now_ms,
        ) {
            Ok(outcome) => outcome,
            Err(PortFailure::Domain(diag)) => {
                store
                    .record_audit(&NewAuditEntry {
                        recorded_at_ms: now_ms,
                        repository_id: policy.repository.repository_id,
                        repository: policy.repository_key(),
                        actor: "unknown".to_string(),
                        trigger_kind: trigger_kind.to_string(),
                        issue_number: 0,
                        comment_id: None,
                        label_name: None,
                        runbook_id: None,
                        outcome: "refused".to_string(),
                        decision_code: diag.code,
                        request_id: None,
                        dispatch_id: None,
                    })
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                store
                    .set_delivery_outcome(&headers.delivery, "refused", now_ms)
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                queue.remove(&delivery.delivery_id).map_err(|_| {
                    infrastructure(
                        "github_implementation_queue",
                        "refused delivery could not be removed from the queue",
                    )
                })?;
                summary.refused += 1;
                summary.drained += 1;
                continue;
            }
            Err(other) => return Err(other),
        };
        match outcome {
            RoutingOutcome::Ignored { .. } => {
                store
                    .set_delivery_outcome(&headers.delivery, "ignored", now_ms)
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                summary.ignored += 1;
                queue.remove(&delivery.delivery_id).map_err(|_| {
                    infrastructure(
                        "github_implementation_queue",
                        "ignored delivery could not be removed from the queue",
                    )
                })?;
                summary.drained += 1;
            }
            RoutingOutcome::Refused(refusal) => {
                store
                    .record_audit(&refusal.audit.to_new_audit_entry())
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                store
                    .set_delivery_outcome(&headers.delivery, "refused", now_ms)
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                summary.refused += 1;
                queue.remove(&delivery.delivery_id).map_err(|_| {
                    infrastructure(
                        "github_implementation_queue",
                        "refused delivery could not be removed from the queue",
                    )
                })?;
                summary.drained += 1;
            }
            RoutingOutcome::Matched(matched) => {
                summary.matched += 1;
                let keep_queued = execute_matched_route(
                    policy,
                    console_base_url,
                    &matched,
                    store,
                    comments,
                    dispatcher,
                    now_ms,
                    &mut summary.dispatched,
                    &mut summary.replayed,
                    &mut summary.refused,
                    &mut summary.dispatch_errors,
                    &mut summary.comments_posted,
                    &mut summary.comment_errors,
                )?;
                if !keep_queued {
                    store
                        .set_delivery_outcome(&headers.delivery, "matched", now_ms)
                        .map_err(|e| infrastructure("github_routing_store", &e))?;
                    queue.remove(&delivery.delivery_id).map_err(|_| {
                        infrastructure(
                            "github_implementation_queue",
                            "routed delivery could not be removed from the queue",
                        )
                    })?;
                    summary.drained += 1;
                }
            }
        }
    }
    Ok(summary)
}

/// Poll GitHub issue comments and open issue labels for runbook routing.
#[allow(clippy::too_many_arguments)]
pub fn poll_runbooks<C, D>(
    policy: &ImplementationIssuePolicy,
    enrollment: &kiln_core::implementation::Enrollment,
    store: &mut RunbookRoutingStore,
    comments: &mut C,
    dispatcher: &mut D,
    since_ms: u64,
    max_pages: u32,
    now_ms: u64,
) -> PortResult<RunbookPollSummary>
where
    C: GithubCommentPort,
    D: RunbookDispatchPort,
{
    if max_pages == 0 || max_pages > crate::poll::MAX_POLL_PAGES {
        return Err(domain(
            "github_poll_page_bound",
            "poll.max_pages",
            "polling page bound is zero or exceeds its maximum",
        ));
    }
    policy.validate()?;
    ensure_repository_enrolled(policy, enrollment)?;
    let Some(routing) = &policy.runbook_routing else {
        return Ok(RunbookPollSummary::default());
    };
    let console_base_url = routing.console_base_url.as_deref();
    let mut summary = RunbookPollSummary::default();

    // 1. Poll issue/PR comments
    if !routing.commands.is_empty() {
        let mut page = 1_u32;
        for _ in 0..max_pages {
            let fetched = comments.list_issue_comments(
                &policy.repository.owner,
                &policy.repository.repository,
                since_ms,
                page,
                crate::poll::MAX_POLL_PAGE_SIZE,
            )?;
            summary.polled_comments += fetched.records.len();
            for record in fetched.records {
                // Only unedited comments (`created_at == updated_at`) match `created`.
                if record.updated_at_ms != record.created_at_ms {
                    summary.ignored += 1;
                    continue;
                }
                let Some(command_id) = parse_runbook_comment_command(&record.body) else {
                    summary.ignored += 1;
                    continue;
                };
                let Some(route) = routing.commands.iter().find(|r| r.command == command_id) else {
                    summary.ignored += 1;
                    continue;
                };
                let actor = WireActor {
                    login: record.actor,
                    actor_type: record.actor_type,
                };
                let outcome = evaluate_comment_route(
                    policy,
                    routing,
                    &enrollment.actors,
                    route,
                    &actor,
                    record.issue_number,
                    record.comment_id,
                    record.author_association.as_deref(),
                    record.is_pull_request,
                    comments,
                    now_ms,
                )?;
                handle_polled_outcome(
                    policy,
                    console_base_url,
                    outcome,
                    store,
                    comments,
                    dispatcher,
                    now_ms,
                    &mut summary,
                )?;
            }
            match fetched.next_page {
                Some(next) if next > page => page = next,
                _ => break,
            }
        }
    }

    // 2. Poll open issues for mapped labels
    if !routing.labels.is_empty() {
        let mut page = 1_u32;
        for _ in 0..max_pages {
            let fetched = comments.list_issues_for_routing(
                &policy.repository.owner,
                &policy.repository.repository,
                since_ms,
                page,
                crate::poll::MAX_POLL_PAGE_SIZE,
            )?;
            summary.polled_issues += fetched.records.len();
            for record in fetched.records {
                if record.state != "open" {
                    summary.ignored += 1;
                    continue;
                }
                for label in &record.labels {
                    let Some(route) = routing.labels.iter().find(|r| &r.label == label) else {
                        continue;
                    };
                    let actor = WireActor {
                        login: record.actor.clone(),
                        actor_type: record.actor_type.clone(),
                    };
                    let outcome = evaluate_label_route(
                        policy,
                        routing,
                        &enrollment.actors,
                        route,
                        &actor,
                        record.number,
                        record.author_association.as_deref(),
                        false,
                        comments,
                        now_ms,
                    )?;
                    handle_polled_outcome(
                        policy,
                        console_base_url,
                        outcome,
                        store,
                        comments,
                        dispatcher,
                        now_ms,
                        &mut summary,
                    )?;
                }
            }
            match fetched.next_page {
                Some(next) if next > page => page = next,
                _ => break,
            }
        }
    }

    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn handle_polled_outcome<C: GithubCommentPort, D: RunbookDispatchPort>(
    policy: &ImplementationIssuePolicy,
    console_base_url: Option<&str>,
    outcome: RoutingOutcome,
    store: &mut RunbookRoutingStore,
    comments: &mut C,
    dispatcher: &mut D,
    now_ms: u64,
    summary: &mut RunbookPollSummary,
) -> PortResult<()> {
    match outcome {
        RoutingOutcome::Ignored { .. } => {
            summary.ignored += 1;
        }
        RoutingOutcome::Refused(refusal) => {
            let _routing_lock = acquire_routing_lock(store)?;
            store
                .record_audit(&refusal.audit.to_new_audit_entry())
                .map_err(|e| infrastructure("github_routing_store", &e))?;
            summary.refused += 1;
        }
        RoutingOutcome::Matched(matched) => {
            let _routing_lock = acquire_routing_lock(store)?;
            summary.matched += 1;
            let _ = execute_matched_route(
                policy,
                console_base_url,
                &matched,
                store,
                comments,
                dispatcher,
                now_ms,
                &mut summary.dispatched,
                &mut summary.replayed,
                &mut summary.refused,
                &mut summary.dispatch_errors,
                &mut summary.comments_posted,
                &mut summary.comment_errors,
            )?;
        }
    }
    Ok(())
}

fn acquire_routing_lock(
    store: &RunbookRoutingStore,
) -> PortResult<kiln_implementer::RunbookRoutingLock> {
    store.acquire_lock().map_err(|msg| {
        if msg.contains("busy") {
            PortFailure::Retryable(kiln_provider_api::InfrastructureFailure {
                code: "github_routing_store_busy".to_string(),
                message: msg,
            })
        } else {
            infrastructure("github_routing_store", &msg)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_matched_route<C: GithubCommentPort, D: RunbookDispatchPort>(
    policy: &ImplementationIssuePolicy,
    console_base_url: Option<&str>,
    matched: &RoutingMatch,
    store: &mut RunbookRoutingStore,
    comments: &mut C,
    dispatcher: &mut D,
    now_ms: u64,
    dispatched_count: &mut usize,
    replayed_count: &mut usize,
    refused_count: &mut usize,
    dispatch_errors: &mut usize,
    comments_posted: &mut usize,
    comment_errors: &mut usize,
) -> PortResult<bool> {
    let reservation = store
        .reserve_dispatch(&NewDispatchReservation {
            source_key: matched.source_key.clone(),
            request_id: matched.request_id.clone(),
            trigger_kind: matched.trigger_kind.as_str().to_string(),
            repository_id: matched.repository_id,
            issue_number: matched.issue_number,
            runbook_id: matched.runbook_id.clone(),
            dispatch_mode: matched.mode.as_str().to_string(),
            now_ms,
        })
        .map_err(|e| infrastructure("github_routing_store", &e))?;

    let (already_dispatched, existing_dispatch_id, existing_rev) = match reservation {
        DispatchReservation::Reserved(_) => (false, None, None),
        DispatchReservation::Existing(record) => match record.dispatch_phase {
            DispatchPhase::Dispatched => (true, record.dispatch_id, record.source_revision),
            DispatchPhase::Failed => {
                *replayed_count += 1;
                return Ok(false);
            }
            DispatchPhase::Reserved => (false, None, None),
        },
    };

    let (dispatch_id, source_revision) = if already_dispatched {
        *replayed_count += 1;
        let id = existing_dispatch_id.ok_or_else(|| {
            infrastructure(
                "github_routing_store",
                "dispatched record missing dispatch_id",
            )
        })?;
        (id, existing_rev)
    } else {
        let dispatch_result: PortResult<(String, Option<String>)> = match matched.mode {
            RunbookRoutingMode::Dispatch => {
                let target = matched.target.clone().ok_or_else(|| {
                    domain(
                        "github_runbook_routing_policy",
                        "target",
                        "dispatch route missing target",
                    )
                })?;
                dispatcher
                    .dispatch_runbook(
                        &matched.runbook_id,
                        &RunbookDispatchPayload {
                            request_id: matched.request_id.clone(),
                            target,
                            params: matched.params.clone(),
                        },
                    )
                    .map(|out| (out.run_id, Some(out.source_revision)))
            }
            RunbookRoutingMode::Fanout => {
                let opts = matched.fanout.clone().unwrap_or_default();
                let mut targets = opts.targets;
                if targets.is_empty()
                    && opts.target_globs.is_empty()
                    && let Some(target) = &matched.target
                {
                    targets.push(target.clone());
                }
                dispatcher
                    .dispatch_fanout(
                        &matched.runbook_id,
                        &RunbookFanoutPayload {
                            request_id: matched.request_id.clone(),
                            targets,
                            target_globs: opts.target_globs,
                            params: matched.params.clone(),
                            max_concurrency: opts.max_concurrency,
                            budget_cap: opts.budget_cap,
                            claim_policy: opts.claim_policy,
                            scope_path: opts.scope_path,
                        },
                    )
                    .map(|out| (out.fanout_id, None))
            }
        };

        match dispatch_result {
            Ok((dispatch_id, rev)) => {
                store
                    .mark_dispatched(&matched.source_key, &dispatch_id, rev.as_deref(), now_ms)
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                store
                    .record_audit(&NewAuditEntry {
                        recorded_at_ms: now_ms,
                        repository_id: matched.repository_id,
                        repository: matched.repository_key.clone(),
                        actor: sanitize_actor(&matched.actor),
                        trigger_kind: matched.trigger_kind.as_str().to_string(),
                        issue_number: matched.issue_number,
                        comment_id: matched.comment_id,
                        label_name: matched.label_name.clone(),
                        runbook_id: Some(matched.runbook_id.clone()),
                        outcome: "matched".to_string(),
                        decision_code: "dispatched".to_string(),
                        request_id: Some(matched.request_id.clone()),
                        dispatch_id: Some(dispatch_id.clone()),
                    })
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                *dispatched_count += 1;
                (dispatch_id, rev)
            }
            Err(PortFailure::Domain(diag)) => {
                store
                    .mark_dispatch_failed(&matched.source_key, &diag.code, now_ms)
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                store
                    .record_audit(&NewAuditEntry {
                        recorded_at_ms: now_ms,
                        repository_id: matched.repository_id,
                        repository: matched.repository_key.clone(),
                        actor: sanitize_actor(&matched.actor),
                        trigger_kind: matched.trigger_kind.as_str().to_string(),
                        issue_number: matched.issue_number,
                        comment_id: matched.comment_id,
                        label_name: matched.label_name.clone(),
                        runbook_id: Some(matched.runbook_id.clone()),
                        outcome: "refused".to_string(),
                        decision_code: diag.code,
                        request_id: Some(matched.request_id.clone()),
                        dispatch_id: None,
                    })
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                *refused_count += 1;
                *dispatch_errors += 1;
                return Ok(false);
            }
            Err(
                PortFailure::Retryable(inf)
                | PortFailure::Unknown(inf)
                | PortFailure::Infrastructure(inf)
                | PortFailure::TimedOut(inf)
                | PortFailure::Cancelled(inf),
            ) => {
                store
                    .record_audit(&NewAuditEntry {
                        recorded_at_ms: now_ms,
                        repository_id: matched.repository_id,
                        repository: matched.repository_key.clone(),
                        actor: sanitize_actor(&matched.actor),
                        trigger_kind: matched.trigger_kind.as_str().to_string(),
                        issue_number: matched.issue_number,
                        comment_id: matched.comment_id,
                        label_name: matched.label_name.clone(),
                        runbook_id: Some(matched.runbook_id.clone()),
                        outcome: "refused".to_string(),
                        decision_code: inf.code,
                        request_id: Some(matched.request_id.clone()),
                        dispatch_id: None,
                    })
                    .map_err(|e| infrastructure("github_routing_store", &e))?;
                *dispatch_errors += 1;
                // Keep queued so retry replays the exact deterministic request_id.
                return Ok(true);
            }
        }
    };

    // Status reply comment is best-effort and idempotent; failure/ambiguity
    // NEVER cancels or alters the canonical dispatched run.
    let reply_reservation = store
        .reserve_reply(&matched.source_key, now_ms)
        .map_err(|e| infrastructure("github_routing_store", &e))?;
    match reply_reservation {
        ReplyReservation::AlreadyPublished { .. } | ReplyReservation::Skipped => {}
        ReplyReservation::NeedsReconciliation => {
            let marker = reply_marker(&matched.request_id);
            match comments.find_issue_comment_by_marker(
                &policy.repository.owner,
                &policy.repository.repository,
                matched.issue_number,
                &marker,
            ) {
                Ok(Some(existing)) => {
                    store
                        .mark_reply_published(&matched.source_key, existing.comment_id, now_ms)
                        .map_err(|e| infrastructure("github_routing_store", &e))?;
                }
                Ok(None) => {
                    post_reply_and_record(
                        policy,
                        console_base_url,
                        matched,
                        &dispatch_id,
                        source_revision.as_deref(),
                        store,
                        comments,
                        now_ms,
                        comments_posted,
                        comment_errors,
                    )?;
                }
                Err(error) => {
                    // Keep reply_phase = "reserved" so every subsequent pass stays
                    // in NeedsReconciliation until marker lookup succeeds!
                    let _ = store.record_reply_reconciliation_error(
                        &matched.source_key,
                        crate::service::error_code(&error),
                        now_ms,
                    );
                    *comment_errors += 1;
                }
            }
        }
        ReplyReservation::ReadyToPost => {
            post_reply_and_record(
                policy,
                console_base_url,
                matched,
                &dispatch_id,
                source_revision.as_deref(),
                store,
                comments,
                now_ms,
                comments_posted,
                comment_errors,
            )?;
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn post_reply_and_record<C: GithubCommentPort>(
    policy: &ImplementationIssuePolicy,
    console_base_url: Option<&str>,
    matched: &RoutingMatch,
    dispatch_id: &str,
    source_revision: Option<&str>,
    store: &mut RunbookRoutingStore,
    comments: &mut C,
    now_ms: u64,
    comments_posted: &mut usize,
    comment_errors: &mut usize,
) -> PortResult<()> {
    let body = format_runbook_reply_comment(
        console_base_url,
        &matched.runbook_id,
        matched.mode,
        dispatch_id,
        matched.target.as_deref(),
        source_revision,
        &matched.request_id,
    )?;
    match comments.post_issue_comment(
        &policy.repository.owner,
        &policy.repository.repository,
        matched.issue_number,
        &body,
    ) {
        Ok(posted) => {
            store
                .mark_reply_published(&matched.source_key, posted.comment_id, now_ms)
                .map_err(|e| infrastructure("github_routing_store", &e))?;
            *comments_posted += 1;
        }
        Err(
            PortFailure::Unknown(inf) | PortFailure::TimedOut(inf) | PortFailure::Cancelled(inf),
        ) => {
            // Keep reply_phase = "reserved" so the next pass reconciles read-only!
            let _ = store.record_reply_reconciliation_error(&matched.source_key, &inf.code, now_ms);
            *comment_errors += 1;
        }
        Err(error) => {
            let _ = store.mark_reply_failed(
                &matched.source_key,
                crate::service::error_code(&error),
                now_ms,
            );
            *comment_errors += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::{
        GithubCommentClient, IssueCommentPollPage, IssueCommentPollRecord, IssueRoutingPollPage,
        IssueRoutingPollRecord, PostedIssueComment,
    };
    use crate::runbook_dispatch::{
        RunbookDispatchOutcome, RunbookFanoutOutcome, WebDispatchConfig, WebRunbookDispatchClient,
    };
    use crate::secret::ResolvedSecret;
    use hmac::{Hmac, Mac};
    use kiln_core::implementation::{AcceptanceObligation, TerminalPolicy, WorkBudget};
    use kiln_core::types::ContentId;
    use kiln_provider_api::InfrastructureFailure;
    use sha2::Sha256;
    use std::io::Write;
    use std::net::TcpListener;

    const SECRET: &[u8] = b"routing-webhook-secret-token";

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn sign(body: &[u8], event: &str, delivery: &str) -> DeliveryHeaders {
        let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).unwrap_or_else(|_| unreachable!());
        mac.update(body);
        DeliveryHeaders {
            delivery: delivery.to_string(),
            event: event.to_string(),
            signature: format!("sha256={}", hex(&mac.finalize().into_bytes())),
        }
    }

    fn routing_policy() -> ImplementationIssuePolicy {
        ImplementationIssuePolicy {
            repository: EventPolicy {
                owner: "OnixResearch".to_string(),
                repository: "kiln".to_string(),
                repository_id: 42,
                allow_fork_pull_requests: false,
                untrusted_pull_requests: Default::default(),
            },
            project: "kiln".to_string(),
            base: None,
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: ContentId::from_blake3_hex("a".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            selection: None,
            runbook_routing: Some(RunbookRoutingPolicy {
                allowed_actors: vec!["maintainer".to_string()],
                allow_issue_comments: true,
                allow_pull_request_comments: true,
                require_push_association: true,
                console_base_url: Some("https://kiln.example.invalid".to_string()),
                labels: vec![
                    LabelRunbookRoute {
                        label: "runbook:flake-update".to_string(),
                        runbook_id: "flake-update".to_string(),
                        target: None,
                        target_ref: Some("refs/heads/main".to_string()),
                        params: BTreeMap::from([(
                            "dry_run".to_string(),
                            RoutingParamValue::Bool(false),
                        )]),
                        mode: RunbookRoutingMode::Dispatch,
                        fanout: None,
                    },
                    LabelRunbookRoute {
                        label: "runbook:fleet-check".to_string(),
                        runbook_id: "fleet-check".to_string(),
                        target: None,
                        target_ref: None,
                        params: BTreeMap::new(),
                        mode: RunbookRoutingMode::Fanout,
                        fanout: Some(RoutingFanoutOptions {
                            targets: vec!["OnixResearch/kiln@refs/heads/main".to_string()],
                            target_globs: Vec::new(),
                            max_concurrency: Some(2),
                            budget_cap: Some(8),
                            claim_policy: Some(RoutingClaimPolicy::Refuse),
                            scope_path: Some("crates/kiln-core".to_string()),
                        }),
                    },
                ],
                commands: vec![CommandRunbookRoute {
                    command: "flake-update".to_string(),
                    runbook_id: "flake-update".to_string(),
                    target: Some("OnixResearch/kiln@refs/heads/main".to_string()),
                    target_ref: None,
                    params: BTreeMap::from([(
                        "secret_path".to_string(),
                        RoutingParamValue::Text("flake.lock".to_string()),
                    )]),
                    mode: RunbookRoutingMode::Dispatch,
                    fanout: None,
                }],
            }),
        }
    }

    fn enrolled() -> BTreeSet<String> {
        BTreeSet::from(["maintainer".to_string()])
    }

    fn enrollment() -> kiln_core::implementation::Enrollment {
        kiln_core::implementation::Enrollment {
            repositories: BTreeSet::from(["onixresearch/kiln".to_string()]),
            actors: enrolled(),
        }
    }

    struct MockGithub {
        pr_head: PortResult<PullRequestHead>,
        polled_comments: Vec<IssueCommentPollRecord>,
        polled_issues: Vec<IssueRoutingPollRecord>,
        posted_comments: Vec<(u64, String)>,
        next_comment_id: u64,
        fail_post_unknown: bool,
        fail_post_timed_out: bool,
        persist_on_ambiguous_post: bool,
        fail_find_marker: bool,
        fail_list_comments: bool,
        reconcile_found: Option<PostedIssueComment>,
    }

    impl MockGithub {
        fn same_repo() -> Self {
            Self {
                pr_head: Ok(PullRequestHead {
                    repo: Some(PullRequestHeadRepo {
                        id: 42,
                        name: "kiln".to_string(),
                        full_name: Some("OnixResearch/kiln".to_string()),
                        owner_login: "OnixResearch".to_string(),
                        fork: false,
                    }),
                }),
                polled_comments: Vec::new(),
                polled_issues: Vec::new(),
                posted_comments: Vec::new(),
                next_comment_id: 900,
                fail_post_unknown: false,
                fail_post_timed_out: false,
                persist_on_ambiguous_post: false,
                fail_find_marker: false,
                fail_list_comments: false,
                reconcile_found: None,
            }
        }
    }

    impl PullRequestOriginCheck for MockGithub {
        fn check_pull_request_head(
            &mut self,
            _owner: &str,
            _repository: &str,
            _pull_request_number: u64,
        ) -> PortResult<PullRequestHead> {
            self.pr_head.clone()
        }
    }

    impl GithubCommentPort for MockGithub {
        fn list_issue_comments(
            &mut self,
            _owner: &str,
            _repository: &str,
            _since_ms: u64,
            page: u32,
            _per_page: u8,
        ) -> PortResult<IssueCommentPollPage> {
            if self.fail_list_comments {
                return Err(infrastructure(
                    "github_comment_poll_unexpected",
                    "list_issue_comments should not be called",
                ));
            }
            Ok(IssueCommentPollPage {
                records: if page == 1 {
                    self.polled_comments.clone()
                } else {
                    Vec::new()
                },
                next_page: None,
            })
        }

        fn list_issues_for_routing(
            &mut self,
            _owner: &str,
            _repository: &str,
            _since_ms: u64,
            page: u32,
            _per_page: u8,
        ) -> PortResult<IssueRoutingPollPage> {
            Ok(IssueRoutingPollPage {
                records: if page == 1 {
                    self.polled_issues.clone()
                } else {
                    Vec::new()
                },
                next_page: None,
            })
        }

        fn post_issue_comment(
            &mut self,
            _owner: &str,
            _repository: &str,
            issue_number: u64,
            body: &str,
        ) -> PortResult<PostedIssueComment> {
            if self.fail_post_unknown {
                if self.persist_on_ambiguous_post {
                    self.next_comment_id += 1;
                    self.posted_comments.push((issue_number, body.to_string()));
                }
                return Err(PortFailure::Unknown(InfrastructureFailure {
                    code: "github_comment_transport_unknown".to_string(),
                    message: "ambiguous".to_string(),
                }));
            }
            if self.fail_post_timed_out {
                if self.persist_on_ambiguous_post {
                    self.next_comment_id += 1;
                    self.posted_comments.push((issue_number, body.to_string()));
                }
                return Err(PortFailure::TimedOut(InfrastructureFailure {
                    code: "github_comment_timed_out".to_string(),
                    message: "timed out".to_string(),
                }));
            }
            self.next_comment_id += 1;
            self.posted_comments.push((issue_number, body.to_string()));
            Ok(PostedIssueComment {
                comment_id: self.next_comment_id,
            })
        }

        fn find_issue_comment_by_marker(
            &mut self,
            _owner: &str,
            _repository: &str,
            _issue_number: u64,
            marker: &str,
        ) -> PortResult<Option<PostedIssueComment>> {
            if self.fail_find_marker {
                return Err(infrastructure("github_comment_http_status", "HTTP 500"));
            }
            if let Some(found) = &self.reconcile_found {
                return Ok(Some(found.clone()));
            }
            Ok(self
                .posted_comments
                .iter()
                .enumerate()
                .find(|(_, (_, body))| body.contains(marker))
                .map(|(idx, _)| PostedIssueComment {
                    comment_id: 901 + idx as u64,
                }))
        }
    }

    #[derive(Default)]
    struct MockDispatcher {
        dispatches: Vec<(String, RunbookDispatchPayload)>,
        fanouts: Vec<(String, RunbookFanoutPayload)>,
        force_grant_denied: bool,
    }

    impl RunbookDispatchPort for MockDispatcher {
        fn dispatch_runbook(
            &mut self,
            runbook_id: &str,
            payload: &RunbookDispatchPayload,
        ) -> PortResult<RunbookDispatchOutcome> {
            if self.force_grant_denied {
                return Err(domain(
                    "runbook_grant_required",
                    "grant",
                    "mutating runbook grant required",
                ));
            }
            self.dispatches
                .push((runbook_id.to_string(), payload.clone()));
            Ok(RunbookDispatchOutcome {
                request_id: payload.request_id.clone(),
                replayed: false,
                runbook_id: runbook_id.to_string(),
                target: payload.target.clone(),
                source_revision: "1111111111111111111111111111111111111111".to_string(),
                run_id: format!("web-run-{}", self.dispatches.len()),
            })
        }

        fn dispatch_fanout(
            &mut self,
            runbook_id: &str,
            payload: &RunbookFanoutPayload,
        ) -> PortResult<RunbookFanoutOutcome> {
            self.fanouts.push((runbook_id.to_string(), payload.clone()));
            Ok(RunbookFanoutOutcome {
                request_id: payload.request_id.clone(),
                replayed: false,
                fanout_id: format!("rb-fanout-{}", self.fanouts.len()),
            })
        }
    }

    #[test]
    fn parse_runbook_comment_command_enforces_exact_grammar() {
        assert_eq!(
            parse_runbook_comment_command("/runbook flake-update"),
            Some("flake-update")
        );
        assert_eq!(
            parse_runbook_comment_command("  \n/runbook\tflake-update \r\n"),
            Some("flake-update")
        );
        for rejected in [
            "",
            "/runbook",
            "/runbook   ",
            "/runbook\nflake-update",
            "/runbook flake-update --target evil@main",
            "/runbook flake-update please",
            "Please run\n/runbook flake-update",
            "/runbook flake-update\nnext line",
            "/runbook-extra flake-update",
            "/runbook Bad_Id",
            "/runbook 123abc",
        ] {
            assert_eq!(parse_runbook_comment_command(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn policy_validation_rejects_invalid_runbook_routing_configs() {
        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .require_push_association = false;
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .allowed_actors = vec!["*".to_string()];
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .allowed_actors = vec!["dependabot[bot]".to_string()];
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .allowed_actors = vec!["dependabot[BOT]".to_string()];
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .console_base_url = Some("http://kiln.example.invalid".to_string());
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .console_base_url = Some("https://kiln.example.invalid/?q=1".to_string());
        assert!(bad.validate().is_err());

        let mut bad = routing_policy();
        bad.runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .console_base_url = Some("https://kiln.example.invalid/a//b".to_string());
        assert!(bad.validate().is_err());
    }

    #[test]
    fn label_and_comment_routing_admit_and_ignore_expected_events() {
        let policy = routing_policy();
        let actors = enrolled();
        let mut gh = MockGithub::same_repo();

        // 1. Open issue + mapped dispatch label -> Matched
        let label_body = serde_json::to_vec(&serde_json::json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "label": {"name": "runbook:flake-update"},
            "issue": {
                "number": 10,
                "state": "open",
                "updated_at": "2026-11-11T10:00:00Z",
                "author_association": "MEMBER"
            }
        }))
        .unwrap_or_else(|_| unreachable!());
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &label_body,
                "issues",
                "00000000-0000-0000-0000-000000000001",
            ),
            &label_body,
            SECRET,
            &mut gh,
            100,
        )
        .unwrap_or_else(|_| unreachable!());
        let RoutingOutcome::Matched(matched) = outcome else {
            unreachable!("expected Matched, got {outcome:?}");
        };
        assert_eq!(matched.runbook_id, "flake-update");
        assert_eq!(matched.mode, RunbookRoutingMode::Dispatch);
        assert_eq!(
            matched.target.as_deref(),
            Some("OnixResearch/kiln@refs/heads/main")
        );
        assert!(matched.request_id.starts_with("gh-rb-"));
        assert_eq!(matched.request_id.len(), 70);

        // 2. Non-fork PR comment `/runbook flake-update` -> Matched
        let pr_comment = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {
                "number": 11,
                "state": "open",
                "updated_at": "2026-11-11T10:01:00Z",
                "pull_request": {"url": "https://api.github.com/repos/OnixResearch/kiln/pulls/11"}
            },
            "comment": {
                "id": 777,
                "body": "  /runbook flake-update  ",
                "user": {"login": "maintainer", "type": "User"},
                "author_association": "OWNER"
            }
        }))
        .unwrap_or_else(|_| unreachable!());
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &pr_comment,
                "issue_comment",
                "00000000-0000-0000-0000-000000000002",
            ),
            &pr_comment,
            SECRET,
            &mut gh,
            101,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(outcome, RoutingOutcome::Matched(_)));

        // 3. Ignored cases: unmapped label, edited comment, prose around command, closed issue
        let ignored_bodies = [
            (
                "issues",
                serde_json::json!({
                    "action": "labeled",
                    "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
                    "sender": {"login": "maintainer"},
                    "label": {"name": "triage"},
                    "issue": {"number": 10, "state": "open", "updated_at": "2026-11-11T10:00:00Z", "author_association": "OWNER"}
                }),
            ),
            (
                "issues",
                serde_json::json!({
                    "action": "labeled",
                    "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
                    "sender": {"login": "maintainer"},
                    "label": {"name": "runbook:flake-update"},
                    "issue": {"number": 10, "state": "closed", "updated_at": "2026-11-11T10:00:00Z", "author_association": "OWNER"}
                }),
            ),
            (
                "issue_comment",
                serde_json::json!({
                    "action": "edited",
                    "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
                    "sender": {"login": "maintainer"},
                    "issue": {"number": 10, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
                    "comment": {"id": 888, "body": "/runbook flake-update", "user": {"login": "maintainer"}, "author_association": "OWNER"}
                }),
            ),
            (
                "issue_comment",
                serde_json::json!({
                    "action": "created",
                    "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
                    "sender": {"login": "stranger"},
                    "issue": {"number": 10, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
                    "comment": {"id": 889, "body": "please /runbook flake-update --flag", "user": {"login": "stranger"}, "author_association": "NONE"}
                }),
            ),
            (
                "issue_comment",
                serde_json::json!({
                    "action": "created",
                    "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
                    "sender": {"login": "maintainer"},
                    "issue": {"number": 10, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
                    "comment": {"id": 890, "body": "/runbook unknown-id", "user": {"login": "maintainer"}, "author_association": "OWNER"}
                }),
            ),
        ];
        for (idx, (event, json_val)) in ignored_bodies.into_iter().enumerate() {
            let bytes = serde_json::to_vec(&json_val).unwrap_or_else(|_| unreachable!());
            let delivery = format!("00000000-0000-0000-0000-00000000001{idx}");
            let outcome = admit_runbook_routing_event(
                &policy,
                &actors,
                &sign(&bytes, event, &delivery),
                &bytes,
                SECRET,
                &mut gh,
                102,
            )
            .unwrap_or_else(|_| unreachable!());
            assert!(
                matches!(outcome, RoutingOutcome::Ignored { .. }),
                "case {idx} should be Ignored, got {outcome:?}"
            );
        }
    }

    #[test]
    fn security_refusals_cover_repo_actor_outside_contributor_and_forks() {
        let policy = routing_policy();
        let actors = enrolled();
        let mut gh = MockGithub::same_repo();

        let comment_payload = |repo_id: u64,
                               sender: &str,
                               user: &str,
                               assoc: Option<&str>,
                               is_pr: bool| {
            serde_json::to_vec(&serde_json::json!({
                "action": "created",
                "repository": {"id": repo_id, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
                "sender": {"login": sender, "type": "User"},
                "issue": {
                    "number": 15,
                    "state": "open",
                    "updated_at": "2026-11-11T10:00:00Z",
                    "pull_request": if is_pr { serde_json::json!({}) } else { serde_json::Value::Null }
                },
                "comment": {
                    "id": 999,
                    "body": "/runbook flake-update",
                    "user": {"login": user, "type": "User"},
                    "author_association": assoc
                }
            }))
            .unwrap_or_else(|_| unreachable!())
        };

        // Wrong repository
        let body = comment_payload(999, "maintainer", "maintainer", Some("OWNER"), false);
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000021",
            ),
            &body,
            SECRET,
            &mut gh,
            200,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(outcome, RoutingOutcome::Refused(r) if r.code == "github_repository_mismatch")
        );

        // Non-allowlisted actor
        let body = comment_payload(42, "outsider", "outsider", Some("COLLABORATOR"), false);
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000022",
            ),
            &body,
            SECRET,
            &mut gh,
            201,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(outcome, RoutingOutcome::Refused(r) if r.code == "github_actor_denied"));

        // Sender != comment.user
        let body = comment_payload(42, "maintainer", "outsider", Some("OWNER"), false);
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000023",
            ),
            &body,
            SECRET,
            &mut gh,
            202,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(outcome, RoutingOutcome::Refused(r) if r.code == "github_actor_denied"));

        // Outside contributor (NONE / CONTRIBUTOR / None)
        for assoc in [
            Some("NONE"),
            Some("CONTRIBUTOR"),
            Some("FIRST_TIME_CONTRIBUTOR"),
            None,
        ] {
            let body = comment_payload(42, "maintainer", "maintainer", assoc, false);
            let outcome = admit_runbook_routing_event(
                &policy,
                &actors,
                &sign(
                    &body,
                    "issue_comment",
                    "00000000-0000-0000-0000-000000000024",
                ),
                &body,
                SECRET,
                &mut gh,
                203,
            )
            .unwrap_or_else(|_| unreachable!());
            assert!(
                matches!(outcome, RoutingOutcome::Refused(ref r) if r.code == "github_outside_contributor_denied"),
                "assoc {assoc:?} got {outcome:?}"
            );
        }

        // Fork PR refused
        gh.pr_head = Ok(PullRequestHead {
            repo: Some(PullRequestHeadRepo {
                id: 9999,
                name: "kiln".to_string(),
                full_name: Some("stranger/kiln".to_string()),
                owner_login: "stranger".to_string(),
                fork: true,
            }),
        });
        let body = comment_payload(42, "maintainer", "maintainer", Some("OWNER"), true);
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000025",
            ),
            &body,
            SECRET,
            &mut gh,
            204,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(outcome, RoutingOutcome::Refused(r) if r.code == "github_fork_denied"));

        // PR origin lookup failure fails closed
        gh.pr_head = Err(infrastructure("github_comment_transport", "lookup failed"));
        let outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000026",
            ),
            &body,
            SECRET,
            &mut gh,
            205,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(outcome, RoutingOutcome::Refused(r) if r.code == "github_pr_origin_unverified")
        );
    }

    #[test]
    fn drain_and_poll_enforce_two_layer_idempotency_and_reply_reconciliation() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 16,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 16)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 16).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();

        let body1 = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 50, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 5001, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h1 = sign(
            &body1,
            "issue_comment",
            "00000000-0000-0000-0000-000000000101",
        );
        queue
            .record(&h1.delivery, &h1.event, &h1.signature, &body1, 1)
            .unwrap_or_else(|_| unreachable!());

        let mut gh = MockGithub::same_repo();
        let mut disp = MockDispatcher::default();
        let sum1 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 1000,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (sum1.dispatched, sum1.comments_posted, sum1.replayed),
            (1, 1, 0)
        );
        assert_eq!(disp.dispatches.len(), 1);
        assert_eq!(gh.posted_comments.len(), 1);
        let comment_text = &gh
            .posted_comments
            .first()
            .unwrap_or_else(|| unreachable!())
            .1;
        assert!(
            comment_text
                .contains("https://kiln.example.invalid/?page=runs&run=web-run-1&view=pipeline")
        );
        assert!(comment_text.contains("<!-- kiln-runbook-dispatch:gh-rb-"));
        assert!(!comment_text.contains("routing-webhook-secret-token"));
        assert!(!comment_text.contains("secret_path"));

        // Distinct delivery_id for the exact same comment_id -> replayed, zero new dispatches/comments!
        let h2 = sign(
            &body1,
            "issue_comment",
            "00000000-0000-0000-0000-000000000102",
        );
        queue
            .record(&h2.delivery, &h2.event, &h2.signature, &body1, 2)
            .unwrap_or_else(|_| unreachable!());
        let sum2 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 1001,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (sum2.dispatched, sum2.replayed, sum2.comments_posted),
            (0, 1, 0)
        );
        assert_eq!(disp.dispatches.len(), 1);
        assert_eq!(gh.posted_comments.len(), 1);

        // Polling the same comment also replays without dispatching or reposting!
        gh.polled_comments.push(IssueCommentPollRecord {
            comment_id: 5001,
            issue_number: 50,
            body: "/runbook flake-update".to_string(),
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            created_at_ms: 100,
            updated_at_ms: 100,
            is_pull_request: false,
        });
        let poll_sum = poll_runbooks(&policy, &enroll, &mut store, &mut gh, &mut disp, 0, 1, 1002)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (
                poll_sum.dispatched,
                poll_sum.replayed,
                poll_sum.comments_posted
            ),
            (0, 1, 0)
        );

        // Same delivery_id with modified body -> audited & dequeued as github_delivery_conflict!
        let mut modified = body1.clone();
        modified.push(b' ');
        let h_conflict = sign(
            &modified,
            "issue_comment",
            "00000000-0000-0000-0000-000000000101",
        );
        queue
            .record(
                &h_conflict.delivery,
                &h_conflict.event,
                &h_conflict.signature,
                &modified,
                3,
            )
            .unwrap_or_else(|_| unreachable!());
        let conflict = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 1003,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((conflict.refused, conflict.drained), (1, 1));
        assert!(queue.is_empty().unwrap_or_else(|_| unreachable!()));
        let audit = store.list_audit(20).unwrap_or_else(|_| unreachable!());
        assert!(
            audit
                .iter()
                .any(|entry| entry.decision_code == "github_delivery_conflict")
        );
    }

    #[test]
    fn ambiguous_comment_post_reconciles_read_only_without_duplicate_comment() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();
        let body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer"},
            "issue": {"number": 51, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 5002, "body": "/runbook flake-update", "user": {"login": "maintainer"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h1 = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000201",
        );
        queue
            .record(&h1.delivery, &h1.event, &h1.signature, &body, 1)
            .unwrap_or_else(|_| unreachable!());

        let mut gh = MockGithub::same_repo();
        gh.fail_post_unknown = true;
        let mut disp = MockDispatcher::default();
        let sum1 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 2000,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((sum1.dispatched, sum1.comment_errors), (1, 1));

        // On subsequent replay, provider actually already created the comment;
        // reconcile finds it via marker and does NOT post a second comment!
        gh.fail_post_unknown = false;
        gh.reconcile_found = Some(PostedIssueComment { comment_id: 9999 });
        let h2 = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000202",
        );
        queue
            .record(&h2.delivery, &h2.event, &h2.signature, &body, 2)
            .unwrap_or_else(|_| unreachable!());
        let sum2 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 2001,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (sum2.dispatched, sum2.replayed, sum2.comments_posted),
            (0, 1, 0)
        );
        assert!(gh.posted_comments.is_empty());
    }

    #[test]
    fn loopback_clients_exercise_real_http_headers_and_payloads() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                let req = crate::tests::read_http_request(&mut stream);
                let resp = if req.starts_with("GET /repos/OnixResearch/kiln/pulls/12 ") {
                    r#"{"head":{"repo":{"id":42,"name":"kiln","full_name":"OnixResearch/kiln","owner":{"login":"OnixResearch"},"fork":false}}}"#
                } else if req.starts_with("POST /repos/OnixResearch/kiln/issues/12/comments ") {
                    r#"{"id":12345}"#
                } else {
                    r#"{"request_id":"gh-rb-test","replayed":false,"runbook_id":"flake-update","target":"OnixResearch/kiln@refs/heads/main","source_revision":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","run_id":"web-run-1"}"#
                };
                let status = if req.contains("POST ") {
                    "201 Created"
                } else {
                    "200 OK"
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
                    resp.len()
                );
                stream
                    .write_all(reply.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
                requests.push(req);
            }
            requests
        });

        let origin = format!("http://{addr}");
        let gh_token =
            ResolvedSecret::from_bytes(b"gh-secret".to_vec()).unwrap_or_else(|_| unreachable!());
        let mut gh = GithubCommentClient::new_fixture(gh_token, 5_000, &origin)
            .unwrap_or_else(|_| unreachable!());
        let head = gh
            .check_pull_request_head("OnixResearch", "kiln", 12)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(head.repo.as_ref().map(|r| r.id), Some(42));
        let posted = gh
            .post_issue_comment("OnixResearch", "kiln", 12, "Kiln dispatched")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(posted.comment_id, 12345);

        let web_token =
            ResolvedSecret::from_bytes(b"web-secret".to_vec()).unwrap_or_else(|_| unreachable!());
        let mut web = WebRunbookDispatchClient::new_fixture(
            WebDispatchConfig {
                service_origin: origin.clone(),
                timeout_ms: 5_000,
                maximum_response_bytes: 65_536,
            },
            web_token,
        )
        .unwrap_or_else(|_| unreachable!());
        let out = web
            .dispatch_runbook(
                "flake-update",
                &RunbookDispatchPayload {
                    request_id: "gh-rb-test".to_string(),
                    target: "OnixResearch/kiln@refs/heads/main".to_string(),
                    params: BTreeMap::new(),
                },
            )
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(out.run_id, "web-run-1");

        let recorded = handle.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(recorded.len(), 3);
        let web_req = recorded.get(2).unwrap_or_else(|| unreachable!());
        assert!(web_req.contains("Authorization: Bearer web-secret"));
        assert!(web_req.contains(&format!("Origin: {origin}")));
    }

    #[test]
    fn drain_and_poll_refuse_unenrolled_repository() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 52, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 5052, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000301",
        );
        queue
            .record(&h.delivery, &h.event, &h.signature, &body, 1)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let unenrolled = kiln_core::implementation::Enrollment {
            repositories: BTreeSet::from(["onixresearch/other".to_string()]),
            actors: enrolled(),
        };
        let mut gh = MockGithub::same_repo();
        let mut disp = MockDispatcher::default();

        let drain_err = drain_runbooks_queue(
            &queue_cfg,
            SECRET,
            &policy,
            &unenrolled,
            &mut store,
            &mut gh,
            &mut disp,
            8,
            3000,
        );
        assert!(
            matches!(drain_err, Err(PortFailure::Domain(ref d)) if d.code == "implementation_unenrolled"),
            "expected implementation_unenrolled, got {drain_err:?}"
        );
        assert_eq!(queue.len().unwrap_or_else(|_| unreachable!()), 1);
        assert!(disp.dispatches.is_empty());

        let poll_err = poll_runbooks(
            &policy,
            &unenrolled,
            &mut store,
            &mut gh,
            &mut disp,
            0,
            1,
            3001,
        );
        assert!(
            matches!(poll_err, Err(PortFailure::Domain(ref d)) if d.code == "implementation_unenrolled"),
            "expected implementation_unenrolled, got {poll_err:?}"
        );
        assert!(disp.dispatches.is_empty());
    }

    #[test]
    fn bot_sender_spoofing_is_refused_for_comment_and_label_events() {
        let policy = routing_policy();
        let actors = enrolled();
        let mut gh = MockGithub::same_repo();

        // issue_comment with sender.type == "Bot" and comment.user.type == "User" must be refused.
        let comment_body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "Bot"},
            "issue": {"number": 16, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 9101, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let comment_outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &comment_body,
                "issue_comment",
                "00000000-0000-0000-0000-000000000311",
            ),
            &comment_body,
            SECRET,
            &mut gh,
            300,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(comment_outcome, RoutingOutcome::Refused(ref r) if r.code == "github_actor_denied"),
            "expected github_actor_denied, got {comment_outcome:?}"
        );

        // issues labeled with sender.type == "Bot" must also be refused.
        let label_body = serde_json::to_vec(&serde_json::json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "Bot"},
            "label": {"name": "runbook:flake-update"},
            "issue": {"number": 16, "state": "open", "updated_at": "2026-11-11T10:00:00Z", "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let label_outcome = admit_runbook_routing_event(
            &policy,
            &actors,
            &sign(
                &label_body,
                "issues",
                "00000000-0000-0000-0000-000000000312",
            ),
            &label_body,
            SECRET,
            &mut gh,
            301,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(label_outcome, RoutingOutcome::Refused(ref r) if r.code == "github_actor_denied"),
            "expected github_actor_denied, got {label_outcome:?}"
        );
    }

    #[test]
    fn poll_runbooks_skips_comment_listing_when_commands_empty() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let routing_path = dir.path().join("routing.sqlite");
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let mut policy = routing_policy();
        policy
            .runbook_routing
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .commands
            .clear();
        let enroll = enrollment();

        let mut gh = MockGithub::same_repo();
        gh.fail_list_comments = true;
        gh.polled_issues.push(IssueRoutingPollRecord {
            number: 25,
            state: "open".to_string(),
            updated_at_ms: 1000,
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            labels: vec!["runbook:flake-update".to_string()],
        });
        let mut disp = MockDispatcher::default();
        let sum = poll_runbooks(&policy, &enroll, &mut store, &mut gh, &mut disp, 0, 1, 2000)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(sum.polled_comments, 0);
        assert_eq!(sum.polled_issues, 1);
        assert_eq!(sum.dispatched, 1);
        assert_eq!(sum.comments_posted, 1);
    }

    #[test]
    fn drain_isolates_poison_deliveries_and_short_circuits_settled_pr_deliveries() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 16,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 16)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 16).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();

        // Pre-seed a settled PR comment delivery in store so we can test both:
        // (1) malformed conflicting body under same delivery_id
        // (2) exact replay of the settled PR comment when PR head lookup fails
        let pr_body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 88, "state": "open", "updated_at": "2026-11-11T10:00:00Z", "pull_request": {}},
            "comment": {"id": 8801, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let pr_headers = sign(
            &pr_body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000401",
        );
        queue
            .record(
                &pr_headers.delivery,
                &pr_headers.event,
                &pr_headers.signature,
                &pr_body,
                1,
            )
            .unwrap_or_else(|_| unreachable!());
        let mut gh = MockGithub::same_repo();
        let mut disp = MockDispatcher::default();
        let seed_sum = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 4000,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(seed_sum.dispatched, 1);

        // Now queue 4 deliveries in order:
        // 1) Bad HMAC delivery
        queue
            .record(
                "00000000-0000-0000-0000-000000000402",
                "issue_comment",
                &format!("sha256={}", "0".repeat(64)),
                b"{}",
                2,
            )
            .unwrap_or_else(|_| unreachable!());
        // 2) Valid HMAC, malformed JSON delivery
        let malformed = b"{not-json";
        let h_malformed = sign(
            malformed,
            "issue_comment",
            "00000000-0000-0000-0000-000000000403",
        );
        queue
            .record(
                &h_malformed.delivery,
                &h_malformed.event,
                &h_malformed.signature,
                malformed,
                3,
            )
            .unwrap_or_else(|_| unreachable!());
        // 3) Valid HMAC, malformed JSON conflicting with settled delivery ...401
        let h_conflict = sign(
            malformed,
            "issue_comment",
            "00000000-0000-0000-0000-000000000401",
        );
        queue
            .record(
                &h_conflict.delivery,
                &h_conflict.event,
                &h_conflict.signature,
                malformed,
                4,
            )
            .unwrap_or_else(|_| unreachable!());
        // 4) Valid deliverable behind the poison rows
        let valid_body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 89, "state": "open", "updated_at": "2026-11-11T10:01:00Z"},
            "comment": {"id": 8901, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h_valid = sign(
            &valid_body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000404",
        );
        queue
            .record(
                &h_valid.delivery,
                &h_valid.event,
                &h_valid.signature,
                &valid_body,
                5,
            )
            .unwrap_or_else(|_| unreachable!());

        let batch_sum = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 4001,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(batch_sum.refused, 3);
        assert_eq!(batch_sum.dispatched, 1);
        assert_eq!(batch_sum.drained, 4);
        assert!(queue.is_empty().unwrap_or_else(|_| unreachable!()));

        let audits = store.list_audit(20).unwrap_or_else(|_| unreachable!());
        assert!(
            audits
                .iter()
                .any(|a| a.decision_code == "github_authentication")
        );
        assert!(
            audits
                .iter()
                .any(|a| a.decision_code == "github_event_payload")
        );
        assert!(
            audits
                .iter()
                .any(|a| a.decision_code == "github_delivery_conflict")
        );

        // Re-queueing the settled PR delivery ...401 when GET /pulls/{n} fails MUST
        // short-circuit locally without calling check_pull_request_head!
        gh.pr_head = Err(infrastructure(
            "github_comment_transport",
            "should not be called",
        ));
        queue
            .record(
                &pr_headers.delivery,
                &pr_headers.event,
                &pr_headers.signature,
                &pr_body,
                6,
            )
            .unwrap_or_else(|_| unreachable!());
        let settled_sum = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 4002,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(settled_sum.ignored, 1);
        assert_eq!(settled_sum.drained, 1);
        assert_eq!(settled_sum.refused, 0);
        assert!(queue.is_empty().unwrap_or_else(|_| unreachable!()));
    }

    #[test]
    fn drain_keeps_delivery_queued_on_408_unknown_and_429_retryable_and_replays_same_request_id() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();

        let body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 60, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 6001, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000501",
        );
        queue
            .record(&h.delivery, &h.event, &h.signature, &body, 1)
            .unwrap_or_else(|_| unreachable!());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let handle = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            let responses = [
                ("408 Request Timeout", r#"{"code":"timeout"}"#),
                ("429 Too Many Requests", r#"{"code":"rate_limited"}"#),
                (
                    "201 Created",
                    r#"{"request_id":"gh-rb-replayed","replayed":false,"runbook_id":"flake-update","target":"OnixResearch/kiln@refs/heads/main","source_revision":"1111111111111111111111111111111111111111","run_id":"web-run-408-429"}"#,
                ),
            ];
            for (status, resp_body) in responses {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                let req = crate::tests::read_http_request(&mut stream);
                bodies.push(req);
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
                stream
                    .write_all(reply.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
            }
            bodies
        });

        let token =
            ResolvedSecret::from_bytes(b"web-secret".to_vec()).unwrap_or_else(|_| unreachable!());
        let mut dispatcher = WebRunbookDispatchClient::new_fixture(
            WebDispatchConfig {
                service_origin: format!("http://{addr}"),
                timeout_ms: 5_000,
                maximum_response_bytes: 65_536,
            },
            token,
        )
        .unwrap_or_else(|_| unreachable!());
        let mut gh = MockGithub::same_repo();

        let sum_408 = drain_runbooks_queue(
            &queue_cfg,
            SECRET,
            &policy,
            &enroll,
            &mut store,
            &mut gh,
            &mut dispatcher,
            8,
            5000,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((sum_408.dispatch_errors, sum_408.drained), (1, 0));
        assert_eq!(queue.len().unwrap_or_else(|_| unreachable!()), 1);

        let sum_429 = drain_runbooks_queue(
            &queue_cfg,
            SECRET,
            &policy,
            &enroll,
            &mut store,
            &mut gh,
            &mut dispatcher,
            8,
            5001,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((sum_429.dispatch_errors, sum_429.drained), (1, 0));
        assert_eq!(queue.len().unwrap_or_else(|_| unreachable!()), 1);

        let sum_201 = drain_runbooks_queue(
            &queue_cfg,
            SECRET,
            &policy,
            &enroll,
            &mut store,
            &mut gh,
            &mut dispatcher,
            8,
            5002,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (sum_201.dispatched, sum_201.comments_posted, sum_201.drained),
            (1, 1, 1)
        );
        assert!(queue.is_empty().unwrap_or_else(|_| unreachable!()));

        let reqs = handle.join().unwrap_or_else(|_| unreachable!());
        let parsed_ids: Vec<String> = reqs
            .iter()
            .map(|raw| {
                let json_str = raw
                    .split_once("\r\n\r\n")
                    .map(|(_, b)| b)
                    .unwrap_or_else(|| unreachable!());
                let val: serde_json::Value =
                    serde_json::from_str(json_str).unwrap_or_else(|_| unreachable!());
                val.get("request_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| unreachable!())
                    .to_string()
            })
            .collect();
        assert_eq!(parsed_ids.len(), 3);
        assert_eq!(parsed_ids.first(), parsed_ids.get(1));
        assert_eq!(parsed_ids.first(), parsed_ids.get(2));
    }

    #[test]
    fn needs_reconciliation_get_failure_and_timed_out_post_keep_reserved_and_avoid_double_post() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();

        // Scenario A: POST persists comment on GitHub then returns Unknown; next pass
        // marker GET fails with 500; third pass marker GET succeeds -> total 1 comment!
        let body_a = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 21, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 2101, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h_a1 = sign(
            &body_a,
            "issue_comment",
            "00000000-0000-0000-0000-000000000601",
        );
        queue
            .record(&h_a1.delivery, &h_a1.event, &h_a1.signature, &body_a, 1)
            .unwrap_or_else(|_| unreachable!());

        let mut gh = MockGithub::same_repo();
        gh.fail_post_unknown = true;
        gh.persist_on_ambiguous_post = true;
        let mut disp = MockDispatcher::default();
        let pass1 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 6000,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((pass1.dispatched, pass1.comment_errors), (1, 1));
        assert_eq!(gh.posted_comments.len(), 1);

        // Pass 2: NeedsReconciliation GET fails with HTTP 500 -> reply_phase stays Reserved!
        gh.fail_post_unknown = false;
        gh.fail_find_marker = true;
        let h_a2 = sign(
            &body_a,
            "issue_comment",
            "00000000-0000-0000-0000-000000000602",
        );
        queue
            .record(&h_a2.delivery, &h_a2.event, &h_a2.signature, &body_a, 2)
            .unwrap_or_else(|_| unreachable!());
        let pass2 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 6001,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (pass2.replayed, pass2.comment_errors, pass2.comments_posted),
            (1, 1, 0)
        );
        let source_key_a = comment_source_key(42, 2101, "flake-update");
        let rec_a = store
            .get_dispatch(&source_key_a)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            rec_a.reply_phase,
            kiln_implementer::runbook_routing::ReplyPhase::Reserved
        );

        // Pass 3: marker lookup succeeds -> reconciles without posting a second comment!
        gh.fail_find_marker = false;
        let h_a3 = sign(
            &body_a,
            "issue_comment",
            "00000000-0000-0000-0000-000000000603",
        );
        queue
            .record(&h_a3.delivery, &h_a3.event, &h_a3.signature, &body_a, 3)
            .unwrap_or_else(|_| unreachable!());
        let pass3 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 6002,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (pass3.replayed, pass3.comment_errors, pass3.comments_posted),
            (1, 0, 0)
        );
        assert_eq!(gh.posted_comments.len(), 1);

        // Scenario B: TimedOut on post_issue_comment also keeps reply_phase == Reserved.
        let body_b = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 22, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 2201, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h_b = sign(
            &body_b,
            "issue_comment",
            "00000000-0000-0000-0000-000000000604",
        );
        queue
            .record(&h_b.delivery, &h_b.event, &h_b.signature, &body_b, 4)
            .unwrap_or_else(|_| unreachable!());
        gh.fail_post_timed_out = true;
        let pass_b = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 6003,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((pass_b.dispatched, pass_b.comment_errors), (1, 1));
        let source_key_b = comment_source_key(42, 2201, "flake-update");
        let rec_b = store
            .get_dispatch(&source_key_b)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            rec_b.reply_phase,
            kiln_implementer::runbook_routing::ReplyPhase::Reserved
        );
    }

    #[test]
    fn poll_label_and_webhook_overlap_survive_updated_at_advancement() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let mut store =
            RunbookRoutingStore::open(&routing_path, 8).unwrap_or_else(|_| unreachable!());
        let policy = routing_policy();
        let enroll = enrollment();

        let mut gh = MockGithub::same_repo();
        let mut disp = MockDispatcher::default();

        // 1. Poll open issue #30 with mapped label at updated_at_ms = 1000.
        gh.polled_issues = vec![IssueRoutingPollRecord {
            number: 30,
            state: "open".to_string(),
            updated_at_ms: 1000,
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            labels: vec!["runbook:flake-update".to_string()],
        }];
        let poll1 = poll_runbooks(&policy, &enroll, &mut store, &mut gh, &mut disp, 0, 1, 7000)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!((poll1.dispatched, poll1.comments_posted), (1, 1));

        // 2. Advance updated_at_ms (reply comment / issue edit) and re-poll -> zero new dispatches/comments!
        gh.polled_issues = vec![IssueRoutingPollRecord {
            number: 30,
            state: "open".to_string(),
            updated_at_ms: 2000,
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            labels: vec!["runbook:flake-update".to_string()],
        }];
        let poll2 = poll_runbooks(&policy, &enroll, &mut store, &mut gh, &mut disp, 0, 1, 7001)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (poll2.dispatched, poll2.replayed, poll2.comments_posted),
            (0, 1, 0)
        );
        assert_eq!(disp.dispatches.len(), 1);
        assert_eq!(gh.posted_comments.len(), 1);

        // 3. Webhook `issues` `labeled` for issue #30 with another timestamp -> replays!
        let label30_body = serde_json::to_vec(&serde_json::json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "label": {"name": "runbook:flake-update"},
            "issue": {"number": 30, "state": "open", "updated_at": "2026-11-11T10:05:00Z", "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h30 = sign(
            &label30_body,
            "issues",
            "00000000-0000-0000-0000-000000000701",
        );
        queue
            .record(&h30.delivery, &h30.event, &h30.signature, &label30_body, 1)
            .unwrap_or_else(|_| unreachable!());
        let drain30 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 7002,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (
                drain30.dispatched,
                drain30.replayed,
                drain30.comments_posted
            ),
            (0, 1, 0)
        );

        // 4. Reverse order on issue #31: webhook first, then poll with newer updated_at_ms -> replays!
        let label31_body = serde_json::to_vec(&serde_json::json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "label": {"name": "runbook:flake-update"},
            "issue": {"number": 31, "state": "open", "updated_at": "2026-11-11T10:06:00Z", "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h31 = sign(
            &label31_body,
            "issues",
            "00000000-0000-0000-0000-000000000702",
        );
        queue
            .record(&h31.delivery, &h31.event, &h31.signature, &label31_body, 2)
            .unwrap_or_else(|_| unreachable!());
        let drain31 = drain_runbooks_queue(
            &queue_cfg, SECRET, &policy, &enroll, &mut store, &mut gh, &mut disp, 8, 7003,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!((drain31.dispatched, drain31.comments_posted), (1, 1));

        gh.polled_issues = vec![IssueRoutingPollRecord {
            number: 31,
            state: "open".to_string(),
            updated_at_ms: 9999,
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            labels: vec!["runbook:flake-update".to_string()],
        }];
        let poll31 = poll_runbooks(&policy, &enroll, &mut store, &mut gh, &mut disp, 0, 1, 7004)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (poll31.dispatched, poll31.replayed, poll31.comments_posted),
            (0, 1, 0)
        );
        assert_eq!(disp.dispatches.len(), 2);
        assert_eq!(gh.posted_comments.len(), 2);
    }

    #[test]
    fn mock_http_reader_reassembles_multi_segment_header_and_body_writes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let body = r#"{"request_id":"gh-rb-split"}"#;
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(addr).unwrap_or_else(|_| unreachable!());
            let seg1 = b"POST /api/runbooks/flake-update/dispatch HTTP/1.1\r\nHost: 127.0.0.1\r\n";
            let seg2 = b"Content-Type: application/json\r\ncOnTeNt-LeNgTh: ";
            let seg3 = format!("{}\r\n\r\n{{\"request_id\":", body.len());
            let seg4 = b"\"gh-rb-split\"}";
            for segment in [
                seg1.as_slice(),
                seg2.as_slice(),
                seg3.as_bytes(),
                seg4.as_slice(),
            ] {
                stream.write_all(segment).unwrap_or_else(|_| unreachable!());
                stream.flush().unwrap_or_else(|_| unreachable!());
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
        let raw = crate::tests::read_http_request(&mut stream);
        client.join().unwrap_or_else(|_| unreachable!());
        let json_str = raw
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .unwrap_or_else(|| unreachable!());
        let val: serde_json::Value =
            serde_json::from_str(json_str).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            val.get("request_id").and_then(|v| v.as_str()),
            Some("gh-rb-split")
        );
    }

    struct SharedSlowGithub {
        inner: std::sync::Arc<std::sync::Mutex<MockGithub>>,
        post_delay: std::time::Duration,
        entered_post_tx: std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>>,
    }

    impl PullRequestOriginCheck for SharedSlowGithub {
        fn check_pull_request_head(
            &mut self,
            owner: &str,
            repository: &str,
            pull_request_number: u64,
        ) -> PortResult<PullRequestHead> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .check_pull_request_head(owner, repository, pull_request_number)
        }
    }

    impl GithubCommentPort for SharedSlowGithub {
        fn list_issue_comments(
            &mut self,
            owner: &str,
            repository: &str,
            since_ms: u64,
            page: u32,
            per_page: u8,
        ) -> PortResult<IssueCommentPollPage> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .list_issue_comments(owner, repository, since_ms, page, per_page)
        }

        fn list_issues_for_routing(
            &mut self,
            owner: &str,
            repository: &str,
            since_ms: u64,
            page: u32,
            per_page: u8,
        ) -> PortResult<IssueRoutingPollPage> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .list_issues_for_routing(owner, repository, since_ms, page, per_page)
        }

        fn post_issue_comment(
            &mut self,
            owner: &str,
            repository: &str,
            issue_number: u64,
            body: &str,
        ) -> PortResult<PostedIssueComment> {
            if let Some(tx) = self
                .entered_post_tx
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .take()
            {
                let _ = tx.send(());
            }
            std::thread::sleep(self.post_delay);
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .post_issue_comment(owner, repository, issue_number, body)
        }

        fn find_issue_comment_by_marker(
            &mut self,
            owner: &str,
            repository: &str,
            issue_number: u64,
            marker: &str,
        ) -> PortResult<Option<PostedIssueComment>> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .find_issue_comment_by_marker(owner, repository, issue_number, marker)
        }
    }

    struct SharedSlowDispatcher {
        inner: std::sync::Arc<std::sync::Mutex<MockDispatcher>>,
    }

    impl RunbookDispatchPort for SharedSlowDispatcher {
        fn dispatch_runbook(
            &mut self,
            runbook_id: &str,
            payload: &RunbookDispatchPayload,
        ) -> PortResult<RunbookDispatchOutcome> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .dispatch_runbook(runbook_id, payload)
        }

        fn dispatch_fanout(
            &mut self,
            runbook_id: &str,
            payload: &RunbookFanoutPayload,
        ) -> PortResult<RunbookFanoutOutcome> {
            self.inner
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .dispatch_fanout(runbook_id, payload)
        }
    }

    #[test]
    fn concurrent_drain_runbooks_processes_serialize_without_double_dispatch_or_double_comment() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 61, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 6101, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000801",
        );
        queue
            .record(&h.delivery, &h.event, &h.signature, &body, 1)
            .unwrap_or_else(|_| unreachable!());
        drop(queue);

        let gh_state = std::sync::Arc::new(std::sync::Mutex::new(MockGithub::same_repo()));
        let disp_state = std::sync::Arc::new(std::sync::Mutex::new(MockDispatcher::default()));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let entered_slot = std::sync::Arc::new(std::sync::Mutex::new(Some(entered_tx)));

        let policy1 = routing_policy();
        let enroll1 = enrollment();
        let qcfg1 = queue_cfg.clone();
        let rpath1 = routing_path.clone();
        let mut gh1 = SharedSlowGithub {
            inner: std::sync::Arc::clone(&gh_state),
            post_delay: std::time::Duration::from_millis(200),
            entered_post_tx: std::sync::Arc::clone(&entered_slot),
        };
        let mut disp1 = SharedSlowDispatcher {
            inner: std::sync::Arc::clone(&disp_state),
        };
        let t1 = std::thread::spawn(move || {
            let mut store =
                RunbookRoutingStore::open(&rpath1, 16).unwrap_or_else(|_| unreachable!());
            drain_runbooks_queue(
                &qcfg1, SECRET, &policy1, &enroll1, &mut store, &mut gh1, &mut disp1, 8, 8000,
            )
            .unwrap_or_else(|_| unreachable!())
        });

        entered_rx.recv().unwrap_or_else(|_| unreachable!());
        let policy2 = routing_policy();
        let enroll2 = enrollment();
        let qcfg2 = queue_cfg;
        let rpath2 = routing_path.clone();
        let mut gh2 = SharedSlowGithub {
            inner: std::sync::Arc::clone(&gh_state),
            post_delay: std::time::Duration::from_millis(200),
            entered_post_tx: std::sync::Arc::clone(&entered_slot),
        };
        let mut disp2 = SharedSlowDispatcher {
            inner: std::sync::Arc::clone(&disp_state),
        };
        let t2 = std::thread::spawn(move || {
            let mut store =
                RunbookRoutingStore::open(&rpath2, 16).unwrap_or_else(|_| unreachable!());
            drain_runbooks_queue(
                &qcfg2, SECRET, &policy2, &enroll2, &mut store, &mut gh2, &mut disp2, 8, 8001,
            )
            .unwrap_or_else(|_| unreachable!())
        });

        let sum1 = t1.join().unwrap_or_else(|_| unreachable!());
        let sum2 = t2.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(sum1.dispatched + sum2.dispatched, 1);
        assert_eq!(sum1.comments_posted + sum2.comments_posted, 1);
        assert_eq!(sum1.dispatch_errors + sum2.dispatch_errors, 0);
        assert_eq!(sum1.comment_errors + sum2.comment_errors, 0);
        assert_eq!(
            disp_state
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .dispatches
                .len(),
            1
        );
        assert_eq!(
            gh_state
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .posted_comments
                .len(),
            1
        );
        let store = RunbookRoutingStore::open(&routing_path, 16).unwrap_or_else(|_| unreachable!());
        let audit = store.list_audit(10).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            audit
                .iter()
                .filter(|entry| entry.decision_code == "dispatched")
                .count(),
            1
        );
    }

    #[test]
    fn concurrent_drain_and_poll_serialize_without_double_dispatch_or_double_comment() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let queue_path = dir.path().join("queue.sqlite");
        let routing_path = dir.path().join("routing.sqlite");
        let queue_cfg = crate::profile::ImplementationQueueConfig {
            path: queue_path.clone(),
            maximum_deliveries: 8,
        };
        let mut queue = kiln_implementer::DeliveryQueue::open(&queue_path, 8)
            .unwrap_or_else(|_| unreachable!());
        let body = serde_json::to_vec(&serde_json::json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 62, "state": "open", "updated_at": "2026-11-11T10:00:00Z"},
            "comment": {"id": 6201, "body": "/runbook flake-update", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }))
        .unwrap_or_else(|_| unreachable!());
        let h = sign(
            &body,
            "issue_comment",
            "00000000-0000-0000-0000-000000000802",
        );
        queue
            .record(&h.delivery, &h.event, &h.signature, &body, 1)
            .unwrap_or_else(|_| unreachable!());
        drop(queue);

        let mut mock_gh = MockGithub::same_repo();
        mock_gh.polled_comments.push(IssueCommentPollRecord {
            comment_id: 6201,
            issue_number: 62,
            body: "/runbook flake-update".to_string(),
            actor: "maintainer".to_string(),
            actor_type: Some("User".to_string()),
            author_association: Some("OWNER".to_string()),
            created_at_ms: 1000,
            updated_at_ms: 1000,
            is_pull_request: false,
        });
        let gh_state = std::sync::Arc::new(std::sync::Mutex::new(mock_gh));
        let disp_state = std::sync::Arc::new(std::sync::Mutex::new(MockDispatcher::default()));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let entered_slot = std::sync::Arc::new(std::sync::Mutex::new(Some(entered_tx)));

        let policy1 = routing_policy();
        let enroll1 = enrollment();
        let rpath1 = routing_path.clone();
        let mut gh1 = SharedSlowGithub {
            inner: std::sync::Arc::clone(&gh_state),
            post_delay: std::time::Duration::from_millis(200),
            entered_post_tx: std::sync::Arc::clone(&entered_slot),
        };
        let mut disp1 = SharedSlowDispatcher {
            inner: std::sync::Arc::clone(&disp_state),
        };
        let t_drain = std::thread::spawn(move || {
            let mut store =
                RunbookRoutingStore::open(&rpath1, 16).unwrap_or_else(|_| unreachable!());
            drain_runbooks_queue(
                &queue_cfg, SECRET, &policy1, &enroll1, &mut store, &mut gh1, &mut disp1, 8, 9000,
            )
            .unwrap_or_else(|_| unreachable!())
        });

        entered_rx.recv().unwrap_or_else(|_| unreachable!());
        let policy2 = routing_policy();
        let enroll2 = enrollment();
        let rpath2 = routing_path.clone();
        let mut gh2 = SharedSlowGithub {
            inner: std::sync::Arc::clone(&gh_state),
            post_delay: std::time::Duration::from_millis(200),
            entered_post_tx: std::sync::Arc::clone(&entered_slot),
        };
        let mut disp2 = SharedSlowDispatcher {
            inner: std::sync::Arc::clone(&disp_state),
        };
        let t_poll = std::thread::spawn(move || {
            let mut store =
                RunbookRoutingStore::open(&rpath2, 16).unwrap_or_else(|_| unreachable!());
            poll_runbooks(
                &policy2, &enroll2, &mut store, &mut gh2, &mut disp2, 0, 1, 9001,
            )
            .unwrap_or_else(|_| unreachable!())
        });

        let drain_sum = t_drain.join().unwrap_or_else(|_| unreachable!());
        let poll_sum = t_poll.join().unwrap_or_else(|_| unreachable!());
        assert_eq!(
            (
                drain_sum.dispatched,
                drain_sum.replayed,
                drain_sum.comments_posted,
                drain_sum.dispatch_errors,
                drain_sum.comment_errors
            ),
            (1, 0, 1, 0, 0)
        );
        assert_eq!(
            (
                poll_sum.dispatched,
                poll_sum.replayed,
                poll_sum.comments_posted,
                poll_sum.dispatch_errors,
                poll_sum.comment_errors
            ),
            (0, 1, 0, 0, 0)
        );
        assert_eq!(
            disp_state
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .dispatches
                .len(),
            1
        );
        assert_eq!(
            gh_state
                .lock()
                .unwrap_or_else(|_| unreachable!())
                .posted_comments
                .len(),
            1
        );
        let store = RunbookRoutingStore::open(&routing_path, 16).unwrap_or_else(|_| unreachable!());
        let audit = store.list_audit(20).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            audit
                .iter()
                .filter(|entry| entry.decision_code == "dispatched")
                .count(),
            1
        );
    }
}
