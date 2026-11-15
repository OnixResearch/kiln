//! Versioned, non-PR native Git publication and integration contract.
//!
//! This is intentionally separate from the v1 PR publication wire format. The
//! core admits exact lineage; the Git shell supplies observations, never Kiln
//! authority.

use alloc::string::{String, ToString};

use serde::{Deserialize, Serialize};

use super::{
    CandidateHandoff, CandidateVerification, ImplementationAttempt, ImplementationWork,
    TerminalPolicy,
};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const NATIVE_GIT_SCHEMA: &str = "kiln.implementation-native-git.v1";
pub const NATIVE_GIT_RECEIPT_SCHEMA: &str = "kiln.implementation-native-git-receipt.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitPlan {
    pub schema: String,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub handoff: ContentId,
    pub verification: ContentId,
    pub review: ContentId,
    pub checks: ContentId,
    pub policy: ContentId,
    pub repository: String,
    pub remote_identity: ContentId,
    pub provider_profile: String,
    /// Native delivery only creates an unoccupied candidate branch.
    pub branch_expected_old: Option<ExternalRef>,
    pub branch_ref: String,
    pub target_ref: String,
    pub expected_old_target: ExternalRef,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub branch_operation: ContentId,
    pub integration_operation: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeGitPhase {
    Prepared,
    BranchPublished,
    IntegrationPending,
    Integrated,
    Unknown,
    Blocked,
}

/// Provider observation remains separate from Kiln's acceptance decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeGitObservation {
    Branch {
        remote: ContentId,
        branch_ref: String,
        commit: ExternalRef,
    },
    Target {
        remote: ContentId,
        target_ref: String,
        previous: ExternalRef,
        resulting: ExternalRef,
    },
    Unknown {
        operation: ContentId,
        reason: String,
    },
    Rejected {
        operation: ContentId,
        code: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitReceipt {
    pub schema: String,
    pub plan: ContentId,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub handoff: ContentId,
    pub verification: ContentId,
    pub review: ContentId,
    pub checks: ContentId,
    pub policy: ContentId,
    pub repository: String,
    pub remote_identity: ContentId,
    pub provider_profile: String,
    pub branch_expected_old: Option<ExternalRef>,
    pub branch_ref: String,
    pub target_ref: String,
    pub expected_old_target: ExternalRef,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub branch_operation: ContentId,
    pub integration_operation: ContentId,
    pub resulting_commit: ExternalRef,
}

/// Admit a native route only for the exact currently verified candidate.
pub fn native_branch_operation_id(plan: &NativeGitPlan) -> CoreResult<ContentId> {
    operation_identity(
        "branch",
        plan,
        &plan.branch_ref,
        plan.branch_expected_old.as_ref(),
    )
}

pub fn native_integration_operation_id(plan: &NativeGitPlan) -> CoreResult<ContentId> {
    operation_identity(
        "integration",
        plan,
        &plan.target_ref,
        Some(&plan.expected_old_target),
    )
}

fn operation_identity(
    kind: &str,
    plan: &NativeGitPlan,
    destination: &str,
    expected_old: Option<&ExternalRef>,
) -> CoreResult<ContentId> {
    let bytes = serde_json::to_vec(&(
        NATIVE_GIT_SCHEMA,
        kind,
        &plan.work_identity,
        plan.work_revision,
        &plan.attempt,
        plan.fence,
        &plan.remote_identity,
        &plan.provider_profile,
        &plan.review,
        &plan.checks,
        &plan.policy,
        destination,
        expected_old,
        &plan.candidate,
        &plan.tree,
    ))
    .map_err(|_| {
        refusal(
            "implementation_native_git_encoding",
            "native_git.operation",
            "cannot encode native Git operation",
        )
    })?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

pub fn admit_native_git_plan(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    verification: &CandidateVerification,
    plan: &NativeGitPlan,
) -> CoreResult<ContentId> {
    let handoff_id = super::admit_candidate_handoff(work, attempt, handoff)?;
    let verification_id =
        super::admit_candidate_verification(work, attempt, handoff, verification)?;
    let valid_ref = |reference: &str| {
        reference.starts_with("refs/")
            && !reference.contains("..")
            && !reference.chars().any(char::is_control)
    };
    if work.terminal_policy != TerminalPolicy::VerifiedIntegrated
        || attempt.phase != super::AttemptPhase::Verified
        || attempt.handoff.as_ref() != Some(handoff)
        || attempt.verification.as_ref() != Some(verification)
        || plan.schema != NATIVE_GIT_SCHEMA
        || plan.work_identity != work.identity()?
        || plan.work_revision != work.revision
        || plan.attempt != attempt.identity
        || plan.fence != attempt.fence
        || plan.handoff != handoff_id
        || plan.verification != verification_id
        || plan.repository != work.repository
        || plan.candidate != handoff.candidate
        || plan.tree != handoff.tree
        || handoff.base.as_ref() != Some(&plan.expected_old_target)
        || plan.branch_expected_old.is_some()
        || !valid_ref(&plan.branch_ref)
        || !valid_ref(&plan.target_ref)
        || plan.branch_ref == plan.target_ref
        || plan.remote_identity.as_str().is_empty()
        || plan.provider_profile.is_empty()
        || plan.review.as_str().is_empty()
        || plan.checks.as_str().is_empty()
        || plan.policy.as_str().is_empty()
        || plan.branch_operation == plan.integration_operation
    {
        return Err(refusal(
            "implementation_native_git_binding",
            "native_git.plan",
            "native Git plan does not bind the exact verified candidate, enrolled repository, distinct refs and current evidence",
        ));
    }
    for reference in [&plan.expected_old_target, &plan.candidate, &plan.tree] {
        reference.validate()?;
    }
    if !commit_ref(&plan.expected_old_target)
        || !commit_ref(&plan.candidate)
        || plan.tree.provider_namespace.as_str() != "git"
        || plan.tree.native_kind.as_str() != "tree"
    {
        return Err(refusal(
            "implementation_native_git_objects",
            "native_git.plan",
            "native Git plan requires exact commit and tree object identities",
        ));
    }
    if plan.branch_operation != native_branch_operation_id(plan)?
        || plan.integration_operation != native_integration_operation_id(plan)?
    {
        return Err(refusal(
            "implementation_native_git_operation",
            "native_git.plan.operation",
            "native Git operation identities do not bind this exact work/remote/ref/candidate",
        ));
    }
    let encoded = serde_json::to_vec(plan).map_err(|_| {
        refusal(
            "implementation_native_git_encoding",
            "native_git.plan",
            "cannot encode native Git plan",
        )
    })?;
    ContentId::from_blake3_hex(blake3::hash(&encoded).to_hex().to_string())
}

/// Shell observations rechecked immediately before receipt admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGitCurrentness {
    pub authority: bool,
    pub policy: bool,
    pub review: bool,
}

/// Admit only a read-back receipt matching the exact prepared plan and current fence.
pub fn admit_native_git_receipt(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    plan: &NativeGitPlan,
    plan_id: &ContentId,
    receipt: &NativeGitReceipt,
    current: NativeGitCurrentness,
) -> CoreResult<ContentId> {
    let plan_bytes = serde_json::to_vec(plan).map_err(|_| {
        refusal(
            "implementation_native_git_encoding",
            "native_git.plan",
            "cannot encode native Git plan",
        )
    })?;
    let expected_plan_id =
        ContentId::from_blake3_hex(blake3::hash(&plan_bytes).to_hex().to_string())?;
    if expected_plan_id != *plan_id {
        return Err(refusal(
            "implementation_native_git_plan_identity",
            "native_git.receipt.plan",
            "native Git receipt references a different plan identity",
        ));
    }
    if !current.authority
        || !current.policy
        || !current.review
        || receipt.schema != NATIVE_GIT_RECEIPT_SCHEMA
        || receipt.plan != *plan_id
        || receipt.work_identity != plan.work_identity
        || receipt.work_identity != work.identity()?
        || receipt.work_revision != work.revision
        || receipt.attempt != attempt.identity
        || receipt.fence != attempt.fence
        || receipt.fence != plan.fence
        || receipt.handoff != plan.handoff
        || receipt.verification != plan.verification
        || receipt.review != plan.review
        || receipt.checks != plan.checks
        || receipt.policy != plan.policy
        || receipt.repository != plan.repository
        || receipt.remote_identity != plan.remote_identity
        || receipt.provider_profile != plan.provider_profile
        || receipt.branch_ref != plan.branch_ref
        || receipt.target_ref != plan.target_ref
        || receipt.branch_expected_old != plan.branch_expected_old
        || receipt.expected_old_target != plan.expected_old_target
        || receipt.candidate != plan.candidate
        || receipt.tree != plan.tree
        || receipt.branch_operation != plan.branch_operation
        || receipt.integration_operation != plan.integration_operation
        || receipt.resulting_commit != plan.candidate
        || !commit_ref(&receipt.resulting_commit)
    {
        return Err(refusal(
            "implementation_native_git_receipt",
            "native_git.receipt",
            "native Git receipt is stale or does not prove the exact operation and remote result",
        ));
    }
    let bytes = serde_json::to_vec(receipt).map_err(|_| {
        refusal(
            "implementation_native_git_encoding",
            "native_git.receipt",
            "cannot encode native Git receipt",
        )
    })?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

fn commit_ref(reference: &ExternalRef) -> bool {
    reference.provider_namespace.as_str() == "git"
        && reference.native_kind.as_str() == "commit"
        && matches!(reference.native_id.as_str().len(), 40 | 64)
        && reference
            .native_id
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_wire_is_distinct_and_observations_do_not_claim_acceptance() {
        assert_ne!(
            NATIVE_GIT_SCHEMA,
            super::super::publication::PUBLICATION_SCHEMA
        );
        assert_ne!(
            NATIVE_GIT_RECEIPT_SCHEMA,
            super::super::publication::INTEGRATION_SCHEMA
        );
        let observation = NativeGitObservation::Unknown {
            operation: ContentId::from_blake3_hex("a".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            reason: "connection lost".into(),
        };
        let bytes = serde_json::to_vec(&observation).unwrap_or_else(|_| unreachable!());
        assert!(String::from_utf8_lossy(&bytes).contains("unknown"));
    }
}
