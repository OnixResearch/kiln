//! Deterministic preparation and remote-state binding for verified-PR delivery.
//!
//! Preparation requires a verified candidate and binds the exact handoff,
//! verification receipt, candidate/tree/base, repository, branch and PR text.
//! Remote publication is a separate shell effect; this module records observed
//! branch/PR identities after that shell reconciles them.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    CandidateHandoff, CandidateVerification, ImplementationWork, TerminalPolicy, content_identity,
    push_str,
};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const PUBLICATION_SCHEMA: &str = "kiln.implementation-publication.v1";
pub const MAX_BRANCH_BYTES: usize = 128;
pub const MAX_TITLE_BYTES: usize = 256;
pub const MAX_BODY_BYTES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationPlan {
    pub schema: String,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub handoff: ContentId,
    pub verification: ContentId,
    pub repository: String,
    pub base: ExternalRef,
    /// Target branch name for the pull request.
    pub base_branch: String,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub branch: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPhase {
    Prepared,
    BranchPublished,
    PullRequestPublished,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRecord {
    pub schema: String,
    pub plan: PublicationPlan,
    pub phase: PublicationPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_head: Option<ExternalRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<ExternalRef>,
}

/// Deterministic implementation branch name for one work/attempt pair.
#[must_use]
pub fn branch_for(work: &ContentId, attempt: &ContentId) -> String {
    format!(
        "kiln/impl-{}-{}",
        work.as_str().chars().take(12).collect::<String>(),
        attempt.as_str().chars().take(12).collect::<String>()
    )
}

/// Admit a publication plan for an exactly verified attempt.
// r[impl kiln.implementation.publication]
pub fn admit_publication_plan(
    work: &ImplementationWork,
    attempt: &super::ImplementationAttempt,
    handoff: &CandidateHandoff,
    verification: &CandidateVerification,
    plan: &PublicationPlan,
) -> CoreResult<ContentId> {
    let handoff_identity = super::admit_candidate_handoff(work, attempt, handoff)?;
    let verification_identity =
        super::admit_candidate_verification(work, attempt, handoff, verification)?;
    let stored = attempt.verification.as_ref().ok_or_else(|| {
        refusal(
            "implementation_publication_verification",
            "publication.verification",
            "publication requires a retained verification receipt",
        )
    })?;
    if attempt.phase != super::AttemptPhase::Verified
        || stored != verification
        || plan.schema != PUBLICATION_SCHEMA
        || plan.work_identity != work.identity()?
        || plan.work_revision != work.revision
        || plan.attempt != attempt.identity
        || plan.fence != attempt.fence
        || plan.handoff != handoff_identity
        || plan.verification != verification_identity
        || plan.repository != work.repository
        || !github_repository(&plan.repository)
        || !bounded_branch(&plan.base_branch)
        || Some(&plan.base) != handoff.base.as_ref()
        || plan.candidate != handoff.candidate
        || plan.tree != handoff.tree
        || plan.branch != branch_for(&work.identity()?, &attempt.identity)
        || work.terminal_policy != TerminalPolicy::VerifiedPullRequest
        || !bounded(&plan.title, MAX_TITLE_BYTES)
        || !bounded(&plan.body, MAX_BODY_BYTES)
    {
        return Err(refusal(
            "implementation_publication_binding",
            "publication.plan",
            "publication plan does not bind the exact verified candidate and branch",
        ));
    }
    plan.base.validate()?;
    plan.candidate.validate()?;
    plan.tree.validate()?;
    let mut bytes = Vec::new();
    push_str(&mut bytes, PUBLICATION_SCHEMA)?;
    for value in [
        plan.work_identity.as_str(),
        plan.attempt.as_str(),
        plan.handoff.as_str(),
        plan.verification.as_str(),
        &plan.repository,
        plan.branch.as_str(),
    ] {
        push_str(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&plan.work_revision.to_be_bytes());
    bytes.extend_from_slice(&plan.fence.to_be_bytes());
    push_str(&mut bytes, &plan.base_branch)?;
    push_external(&mut bytes, &plan.base)?;
    push_external(&mut bytes, &plan.candidate)?;
    push_external(&mut bytes, &plan.tree)?;
    push_str(&mut bytes, &plan.title)?;
    push_str(&mut bytes, &plan.body)?;
    content_identity(&bytes)
}

fn push_external(bytes: &mut Vec<u8>, external: &ExternalRef) -> CoreResult<()> {
    push_str(bytes, external.provider_namespace.as_str())?;
    push_str(bytes, external.native_kind.as_str())?;
    push_str(bytes, external.native_id.as_str())
}

fn github_repository(value: &str) -> bool {
    let Some((owner, repository)) = value.split_once('/') else {
        return false;
    };
    owner.len() <= 100
        && repository.len() <= 100
        && !owner.is_empty()
        && !repository.is_empty()
        && !repository.contains('/')
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && repository
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn bounded_branch(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BRANCH_BYTES
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("..")
        && !value.chars().any(char::is_control)
}

fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_review::{CiCheck, CiFact};
    use crate::implementation::{
        AcceptanceObligation, AttemptPhase, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence,
        EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt,
        ImplementationWork, SourceRef, VERIFICATION_SCHEMA, WorkBudget, WorkSourceKind,
    };
    use alloc::collections::BTreeSet;
    use alloc::string::ToString;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: Some(
                ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
            ),
            title: "Publication work".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#47")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#47".to_string(),
                version: content('1'),
            }],
        }
    }

    fn make_attempt(work: &ImplementationWork) -> ImplementationAttempt {
        let handoff = handoff(work);
        let verification = verification(work, &handoff);
        ImplementationAttempt {
            schema: crate::implementation::ATTEMPT_SCHEMA.to_string(),
            identity: content('a'),
            canonical_key: content('b'),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            base: work.base.clone(),
            ordinal: 1,
            fence: 4,
            lease_until_ms: 100,
            claimed_at_ms: 10,
            ended_at_ms: Some(20),
            phase: AttemptPhase::Verified,
            dispatch: None,
            workflow: Some(
                ExternalRef::new("lattice", "run", "one").unwrap_or_else(|_| unreachable!()),
            ),
            candidate: Some(handoff.candidate.clone()),
            handoff: Some(handoff),
            verification: Some(verification),
            publication: None,
            metrics: None,
            failure: None,
            cause: None,
            superseded_by: None,
        }
    }

    fn handoff(work: &ImplementationWork) -> CandidateHandoff {
        CandidateHandoff {
            schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: content('a'),
            fence: 4,
            repository: work.repository.clone(),
            base: work.base.clone(),
            candidate: ExternalRef::new("git", "commit", "candidate")
                .unwrap_or_else(|_| unreachable!()),
            tree: ExternalRef::new("git", "tree", "tree").unwrap_or_else(|_| unreachable!()),
            changed_paths: vec!["crates/src/lib.rs".to_string()],
            acceptance_digest: crate::implementation::acceptance_digest(work)
                .unwrap_or_else(|_| unreachable!()),
            verification_profile: work.verification_profile.clone(),
            toolchain: content('7'),
            environment: content('8'),
            evidence: vec![CandidateEvidence {
                identity: content('e'),
                kind: EvidenceKind::Check,
                outcome: EvidenceOutcome::Passed,
                reference: ExternalRef::new("kiln", "run", "candidate-check")
                    .unwrap_or_else(|_| unreachable!()),
            }],
        }
    }

    fn verification(
        work: &ImplementationWork,
        handoff: &CandidateHandoff,
    ) -> CandidateVerification {
        CandidateVerification {
            schema: VERIFICATION_SCHEMA.to_string(),
            handoff: crate::implementation::admit_candidate_handoff(
                work,
                &attempt_stub(work, handoff),
                handoff,
            )
            .unwrap_or_else(|_| unreachable!()),
            work_identity: handoff.work_identity.clone(),
            work_revision: handoff.work_revision,
            attempt: handoff.attempt_identity.clone(),
            fence: handoff.fence,
            candidate: handoff.candidate.clone(),
            tree: handoff.tree.clone(),
            base: handoff.base.clone(),
            verification_profile: work.verification_profile.clone(),
            run: CiFact {
                project: work.project.clone(),
                repository: work.repository.clone(),
                revision: handoff.candidate.native_id.as_str().to_string(),
                run_id: "run-1".to_string(),
                ordinal: 1,
                success: true,
                full_run: true,
                clean: true,
                scope_identity: None,
                terminal: true,
                untrusted: None,
                checks: vec![CiCheck {
                    name: "test".to_string(),
                    kind: "test".to_string(),
                    outcome: "pass".to_string(),
                    artifact_digest: Some("f".repeat(64)),
                }],
            },
        }
    }

    fn attempt_stub(
        work: &ImplementationWork,
        handoff: &CandidateHandoff,
    ) -> ImplementationAttempt {
        ImplementationAttempt {
            schema: crate::implementation::ATTEMPT_SCHEMA.to_string(),
            identity: content('a'),
            canonical_key: content('b'),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            base: work.base.clone(),
            ordinal: 1,
            fence: 4,
            lease_until_ms: 100,
            claimed_at_ms: 10,
            ended_at_ms: Some(20),
            phase: AttemptPhase::Verified,
            dispatch: None,
            workflow: None,
            candidate: Some(handoff.candidate.clone()),
            handoff: Some(handoff.clone()),
            verification: None,
            publication: None,
            metrics: None,
            failure: None,
            cause: None,
            superseded_by: None,
        }
    }

    fn make_plan(work: &ImplementationWork, attempt: &ImplementationAttempt) -> PublicationPlan {
        PublicationPlan {
            schema: PUBLICATION_SCHEMA.to_string(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            handoff: crate::implementation::admit_candidate_handoff(
                work,
                attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!()),
            verification: crate::implementation::admit_candidate_verification(
                work,
                attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
                attempt
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!()),
            repository: work.repository.clone(),
            base: work.base.clone().unwrap_or_else(|| unreachable!()),
            base_branch: "main".to_string(),
            candidate: attempt.candidate.clone().unwrap_or_else(|| unreachable!()),
            tree: attempt
                .handoff
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .tree
                .clone(),
            branch: branch_for(
                &work.identity().unwrap_or_else(|_| unreachable!()),
                &attempt.identity,
            ),
            title: "Implement the work".to_string(),
            body: "Exact verified candidate".to_string(),
        }
    }

    #[test]
    // r[verify kiln.implementation.publication] r[verify kiln.implementation.publication.scenario.prepare]
    fn prepares_verified_candidate_publication_with_deterministic_branch() {
        let work = work();
        let attempt = make_attempt(&work);
        let plan = make_plan(&work, &attempt);
        let identity = admit_publication_plan(
            &work,
            &attempt,
            attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
            attempt
                .verification
                .as_ref()
                .unwrap_or_else(|| unreachable!()),
            &plan,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            identity,
            admit_publication_plan(
                &work,
                &attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
                attempt
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                &plan
            )
            .unwrap_or_else(|_| unreachable!())
        );
    }

    #[test]
    // r[verify kiln.implementation.publication] r[verify kiln.implementation.publication.scenario.refused] r[verify kiln.implementation.publication_target] r[verify kiln.implementation.publication_target.scenario.refused]
    fn refuses_unverified_or_wrong_branch_publication_plans() {
        let work = work();
        let attempt = make_attempt(&work);
        let mut plan = make_plan(&work, &attempt);
        plan.branch = "arbitrary".to_string();
        assert!(
            admit_publication_plan(
                &work,
                &attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
                attempt
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                &plan
            )
            .is_err()
        );
        let mut invalid_repository = make_plan(&work, &attempt);
        invalid_repository.repository = "owner/repo/extra".to_string();
        assert!(
            admit_publication_plan(
                &work,
                &attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
                attempt
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                &invalid_repository
            )
            .is_err()
        );
        let mut invalid_base = make_plan(&work, &attempt);
        invalid_base.base_branch = "../main".to_string();
        assert!(
            admit_publication_plan(
                &work,
                &attempt,
                attempt.handoff.as_ref().unwrap_or_else(|| unreachable!()),
                attempt
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                &invalid_base
            )
            .is_err()
        );
        let mut unverified = make_attempt(&work);
        unverified.phase = AttemptPhase::Verifying;
        let plan = make_plan(&work, &unverified);
        assert!(
            admit_publication_plan(
                &work,
                &unverified,
                unverified
                    .handoff
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                unverified
                    .verification
                    .as_ref()
                    .unwrap_or_else(|| unreachable!()),
                &plan
            )
            .is_err()
        );
    }
}
