//! Untrusted pull-request runs: pure classification, approval, supersession,
//! execution grants and required-CI eligibility. Shells own provider facts,
//! principals, clocks and durable storage.
//!
//! r[impl kiln.untrusted.classification]
//! r[impl kiln.untrusted.approval]
//! r[impl kiln.untrusted.execution]

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Provider associations that evidence push access to the base repository.
pub const PUSH_ACCESS_ASSOCIATIONS: [&str; 3] = ["OWNER", "MEMBER", "COLLABORATOR"];
/// Exact commit ids: SHA-1 (40) or SHA-256 (64) lowercase hexadecimal.
pub const COMMIT_HEX_LENGTHS: [usize; 2] = [40, 64];
/// Bound on approver identities and labels retained in durable records.
pub const MAX_APPROVER_TEXT_BYTES: usize = 256;
/// Bound on the retained native head repository name (`owner/name`).
pub const MAX_HEAD_REPOSITORY_BYTES: usize = 512;

/// Reviewed repository policy for pull requests that are not trusted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UntrustedPullRequests {
    /// Refuse untrusted pull requests exactly as before this policy existed.
    #[default]
    Deny,
    /// Hold untrusted pull requests in a durable blocked run until approved.
    Approve,
}
impl UntrustedPullRequests {
    pub fn is_deny(&self) -> bool {
        *self == Self::Deny
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UntrustedReason {
    /// The head repository is not the policy repository.
    Fork,
    /// Same repository, but the author has no evidenced push access.
    AuthorWithoutPushAccess,
}
impl UntrustedReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fork => "fork",
            Self::AuthorWithoutPushAccess => "author_without_push_access",
        }
    }
}

/// Authenticated provider facts about one pull request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullRequestTrustFacts<'a> {
    pub same_repository: bool,
    pub author_association: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustDecision {
    Trusted,
    Refused,
    Untrusted(UntrustedReason),
}

pub fn author_has_push_access(association: Option<&str>) -> bool {
    association.is_some_and(|value| PUSH_ACCESS_ASSOCIATIONS.contains(&value))
}

/// `deny` keeps today's behaviour exactly: forks are refused and same-repository
/// pull requests are trusted without consulting the author association.
pub fn classify(policy: UntrustedPullRequests, facts: PullRequestTrustFacts<'_>) -> TrustDecision {
    match (policy, facts.same_repository) {
        (UntrustedPullRequests::Deny, true) => TrustDecision::Trusted,
        (UntrustedPullRequests::Deny, false) => TrustDecision::Refused,
        (UntrustedPullRequests::Approve, false) => TrustDecision::Untrusted(UntrustedReason::Fork),
        (UntrustedPullRequests::Approve, true) => {
            if author_has_push_access(facts.author_association) {
                TrustDecision::Trusted
            } else {
                TrustDecision::Untrusted(UntrustedReason::AuthorWithoutPushAccess)
            }
        }
    }
}

pub fn valid_commit(value: &str) -> bool {
    COMMIT_HEX_LENGTHS.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Authority the shell established for the approving principal. Only an
/// interactive operator may approve; automation principals never may, even
/// when they hold operator capabilities internally.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApproverAuthority {
    Operator,
    Viewer,
    Worker,
    Automation,
}
/// The single role permitted to approve untrusted runs.
pub const APPROVER_AUTHORITY: ApproverAuthority = ApproverAuthority::Operator;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approver {
    pub id: String,
    pub label: String,
    pub authority: ApproverAuthority,
}

/// The approval command. `source_commit` is the exact commit the approver saw.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproveUntrustedRun {
    pub actor: Approver,
    pub run: String,
    pub source_commit: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntrustedApproval {
    pub actor_id: String,
    pub actor_label: String,
    pub source_commit: String,
    pub approved_at_ms: u64,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Durable gate bound to one run and one exact source commit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntrustedGate {
    pub reason: UntrustedReason,
    pub pull_request: u64,
    pub head_repository: String,
    pub source_commit: String,
    /// Reviewed policy, bound at creation: may this result satisfy required CI.
    #[serde(default, skip_serializing_if = "is_false")]
    pub satisfies_required_ci: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<UntrustedApproval>,
    /// Newer run for the same pull request that superseded this unapproved one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UntrustedState {
    AwaitingApproval,
    Approved,
    Superseded,
}
impl UntrustedState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AwaitingApproval => "awaiting_approval",
            Self::Approved => "approved",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateRejection {
    InvalidCommit,
    InvalidHeadRepository,
    ZeroPullRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalRejection {
    Unauthorized,
    InvalidActor,
    RunMismatch,
    InvalidCommit,
    StaleCommit,
    Superseded,
    AlreadyApproved,
}
impl ApprovalRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "untrusted_approval_unauthorized",
            Self::InvalidActor => "untrusted_approval_actor_invalid",
            Self::RunMismatch => "untrusted_approval_run_mismatch",
            Self::InvalidCommit => "untrusted_approval_commit_invalid",
            Self::StaleCommit => "untrusted_approval_stale_commit",
            Self::Superseded => "untrusted_run_superseded",
            Self::AlreadyApproved => "untrusted_run_not_awaiting_approval",
        }
    }
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APPROVER_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

impl UntrustedGate {
    pub fn new(
        reason: UntrustedReason,
        pull_request: u64,
        head_repository: String,
        source_commit: String,
        satisfies_required_ci: bool,
    ) -> Result<Self, GateRejection> {
        if pull_request == 0 {
            return Err(GateRejection::ZeroPullRequest);
        }
        if head_repository.is_empty()
            || head_repository.len() > MAX_HEAD_REPOSITORY_BYTES
            || head_repository.chars().any(char::is_control)
        {
            return Err(GateRejection::InvalidHeadRepository);
        }
        if !valid_commit(&source_commit) {
            return Err(GateRejection::InvalidCommit);
        }
        Ok(Self {
            reason,
            pull_request,
            head_repository,
            source_commit,
            satisfies_required_ci,
            approval: None,
            superseded_by: None,
        })
    }

    pub fn state(&self) -> UntrustedState {
        match (&self.approval, &self.superseded_by) {
            (Some(_), _) => UntrustedState::Approved,
            (None, Some(_)) => UntrustedState::Superseded,
            (None, None) => UntrustedState::AwaitingApproval,
        }
    }

    /// Only an approval of this gate's exact commit releases execution.
    pub fn may_dispatch(&self) -> bool {
        self.approval
            .as_ref()
            .is_some_and(|approval| approval.source_commit == self.source_commit)
    }

    /// Required CI only for the exact approved commit, and only under policy.
    pub fn satisfies_required_ci(&self, revision: &str) -> bool {
        self.satisfies_required_ci && self.may_dispatch() && self.source_commit == revision
    }

    pub fn summary(&self) -> UntrustedSummary {
        UntrustedSummary {
            state: self.state(),
            reason: self.reason,
            pull_request: self.pull_request,
            head_repository: self.head_repository.clone(),
            source_commit: self.source_commit.clone(),
            approved_by: self.approval.as_ref().map(|a| a.actor_label.clone()),
            approved_by_id: self.approval.as_ref().map(|a| a.actor_id.clone()),
            approved_commit: self.approval.as_ref().map(|a| a.source_commit.clone()),
            approved_at_ms: self.approval.as_ref().map(|a| a.approved_at_ms),
            counts_for_required_ci: self.satisfies_required_ci(&self.source_commit),
            superseded_by: self.superseded_by.clone(),
        }
    }
}

/// Display and review projection of one gate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntrustedSummary {
    pub state: UntrustedState,
    pub reason: UntrustedReason,
    pub pull_request: u64,
    pub head_repository: String,
    pub source_commit: String,
    pub approved_by: Option<String>,
    pub approved_by_id: Option<String>,
    pub approved_commit: Option<String>,
    pub approved_at_ms: Option<u64>,
    pub counts_for_required_ci: bool,
    pub superseded_by: Option<String>,
}

/// Decide one approval. Authorization precedes every other check, so an
/// unauthorized principal learns nothing about run or commit state.
pub fn decide_approval(
    run_id: &str,
    gate: &UntrustedGate,
    command: &ApproveUntrustedRun,
    approved_at_ms: u64,
) -> Result<UntrustedApproval, ApprovalRejection> {
    if command.actor.authority != APPROVER_AUTHORITY {
        return Err(ApprovalRejection::Unauthorized);
    }
    if !bounded_text(&command.actor.id) || !bounded_text(&command.actor.label) {
        return Err(ApprovalRejection::InvalidActor);
    }
    if command.run != run_id {
        return Err(ApprovalRejection::RunMismatch);
    }
    if !valid_commit(&command.source_commit) {
        return Err(ApprovalRejection::InvalidCommit);
    }
    match gate.state() {
        UntrustedState::Superseded => return Err(ApprovalRejection::Superseded),
        UntrustedState::Approved => return Err(ApprovalRejection::AlreadyApproved),
        UntrustedState::AwaitingApproval => {}
    }
    if command.source_commit != gate.source_commit {
        return Err(ApprovalRejection::StaleCommit);
    }
    Ok(UntrustedApproval {
        actor_id: command.actor.id.clone(),
        actor_label: command.actor.label.clone(),
        source_commit: command.source_commit.clone(),
        approved_at_ms,
    })
}

/// A newer gate supersedes an older one for the same pull request and head
/// repository at a different commit, unless the older one was approved (its
/// approval stays bound to its own exact commit).
pub fn supersedes(newer: &UntrustedGate, older: &UntrustedGate) -> bool {
    newer.pull_request == older.pull_request
        && newer.head_repository == older.head_repository
        && newer.source_commit != older.source_commit
        && older.state() == UntrustedState::AwaitingApproval
}

/// Credentials and cache an execution may receive. Untrusted executions get
/// none of them; there is no per-run override.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionGrants {
    pub secrets: bool,
    pub workload_token: bool,
    pub cache_restore: bool,
    pub cache_publish: bool,
    pub requires_untrusted_capable_runner: bool,
}
pub const UNTRUSTED_GRANTS: ExecutionGrants = ExecutionGrants {
    secrets: false,
    workload_token: false,
    cache_restore: false,
    cache_publish: false,
    requires_untrusted_capable_runner: true,
};
pub const TRUSTED_GRANTS: ExecutionGrants = ExecutionGrants {
    secrets: true,
    workload_token: true,
    cache_restore: true,
    cache_publish: true,
    requires_untrusted_capable_runner: false,
};
pub fn execution_grants(untrusted: bool) -> ExecutionGrants {
    if untrusted {
        UNTRUSTED_GRANTS
    } else {
        TRUSTED_GRANTS
    }
}

/// Provenance a control plane attaches to an execution request of an approved
/// untrusted run (and each of its retries).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntrustedProvenance {
    pub reason: UntrustedReason,
    pub source_commit: String,
    pub approver_id: String,
}
impl UntrustedProvenance {
    pub fn admit(&self) -> bool {
        valid_commit(&self.source_commit) && bounded_text(&self.approver_id)
    }
}

#[cfg(test)]
mod tests;
