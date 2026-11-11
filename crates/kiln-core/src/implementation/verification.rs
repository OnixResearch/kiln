//! Candidate verification evidence admitted from retained canonical CI facts.
//!
//! The verifier shell supplies the actual retained engine observation. This
//! module binds it to the exact candidate handoff, work/version and attempt/fence,
//! refuses partial/failed/untrusted evidence, and returns a deterministic
//! receipt identity. It does not execute checks or create CI facts.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{CandidateHandoff, ImplementationWork, content_identity, push_str};
use crate::change_review::{CiCheck, CiFact};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const VERIFICATION_SCHEMA: &str = "kiln.implementation-verification.v1";
pub const MAX_VERIFICATION_CHECKS: usize = 64;
pub const MAX_CHECK_NAME_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateVerification {
    pub schema: String,
    pub handoff: ContentId,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<ExternalRef>,
    pub verification_profile: ContentId,
    pub run: CiFact,
}

/// Admit one retained CI fact as verification for the exact candidate handoff.
///
/// Returns the deterministic verification identity. The caller must supply
/// canonical engine evidence; this module does not execute or fetch CI.
// r[impl kiln.implementation.verification]
pub fn admit_candidate_verification(
    work: &ImplementationWork,
    attempt: &super::ImplementationAttempt,
    handoff: &CandidateHandoff,
    verification: &CandidateVerification,
) -> CoreResult<ContentId> {
    let handoff_identity = super::admit_candidate_handoff(work, attempt, handoff)?;
    if verification.schema != VERIFICATION_SCHEMA
        || verification.handoff != handoff_identity
        || verification.work_identity != handoff.work_identity
        || verification.work_revision != handoff.work_revision
        || verification.attempt != handoff.attempt_identity
        || verification.fence != handoff.fence
        || verification.candidate != handoff.candidate
        || verification.tree != handoff.tree
        || verification.base != handoff.base
        || verification.verification_profile != work.verification_profile
        || verification.run.project != work.project
        || verification.run.repository != work.repository
        || verification.run.revision != handoff.candidate.native_id.as_str()
        || verification.run.ordinal == 0
        || !verification.run.terminal
        || !verification.run.success
        || !verification.run.full_run
        || !verification.run.clean
        || verification.run.untrusted.is_some()
        || verification.run.checks.is_empty()
        || verification.run.checks.len() > MAX_VERIFICATION_CHECKS
    {
        return Err(refusal(
            "implementation_verification_binding",
            "verification.receipt",
            "verification evidence does not bind the exact candidate, work or attempt",
        ));
    }
    let mut checks = verification.run.checks.clone();
    checks.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.outcome.cmp(&right.outcome))
            .then_with(|| left.artifact_digest.cmp(&right.artifact_digest))
    });
    let mut names = alloc::collections::BTreeSet::new();
    for check in &checks {
        if !identifier(&check.name, MAX_CHECK_NAME_BYTES)
            || !identifier(&check.kind, 64)
            || check.outcome != "pass"
            || !names.insert(check.name.as_str())
            || check
                .artifact_digest
                .as_deref()
                .is_some_and(|digest| !hex(digest))
        {
            return Err(refusal(
                "implementation_verification_checks",
                "verification.run.checks",
                "verification checks are empty, duplicated or not all passing",
            ));
        }
    }
    let mut bytes = Vec::new();
    push_str(&mut bytes, VERIFICATION_SCHEMA)?;
    for value in [
        verification.handoff.as_str(),
        verification.work_identity.as_str(),
    ] {
        push_str(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&verification.work_revision.to_be_bytes());
    push_str(&mut bytes, verification.attempt.as_str())?;
    bytes.extend_from_slice(&verification.fence.to_be_bytes());
    push_external(&mut bytes, Some(&verification.candidate))?;
    push_external(&mut bytes, Some(&verification.tree))?;
    push_external(&mut bytes, verification.base.as_ref())?;
    push_str(&mut bytes, verification.verification_profile.as_str())?;
    push_str(&mut bytes, &verification.run.project)?;
    push_str(&mut bytes, &verification.run.repository)?;
    push_str(&mut bytes, &verification.run.revision)?;
    push_str(&mut bytes, &verification.run.run_id)?;
    bytes.extend_from_slice(&verification.run.ordinal.to_be_bytes());
    bytes.push(u8::from(verification.run.terminal));
    bytes.push(u8::from(verification.run.success));
    bytes.push(u8::from(verification.run.full_run));
    bytes.push(u8::from(verification.run.clean));
    if let Some(scope) = &verification.run.scope_identity {
        push_str(&mut bytes, scope)?;
    }
    for check in checks {
        push_check(&mut bytes, &check)?;
    }
    content_identity(&bytes)
}

fn push_check(bytes: &mut Vec<u8>, check: &CiCheck) -> CoreResult<()> {
    push_str(bytes, &check.name)?;
    push_str(bytes, &check.kind)?;
    push_str(bytes, &check.outcome)?;
    if let Some(digest) = &check.artifact_digest {
        push_str(bytes, digest)?;
    }
    Ok(())
}

fn push_external(bytes: &mut Vec<u8>, external: Option<&ExternalRef>) -> CoreResult<()> {
    bytes.push(u8::from(external.is_some()));
    if let Some(external) = external {
        push_str(bytes, external.provider_namespace.as_str())?;
        push_str(bytes, external.native_kind.as_str())?;
        push_str(bytes, external.native_id.as_str())?;
    }
    Ok(())
}

fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_review::CiCheck;
    use crate::implementation::{
        AcceptanceObligation, AttemptLedger, AttemptPhase, CandidateEvidence, CandidateHandoff,
        DEFAULT_LEASE_MS, EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA,
        ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
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
            base: None,
            title: "Verification work".to_string(),
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
                source: ExternalRef::new("github", "issue", "onix/kiln#46")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#46".to_string(),
                version: content('1'),
            }],
        }
    }

    fn handoff(
        work: &ImplementationWork,
        attempt: &super::super::ImplementationAttempt,
    ) -> CandidateHandoff {
        CandidateHandoff {
            schema: crate::implementation::CANDIDATE_HANDOFF_SCHEMA.to_string(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: None,
            candidate: ExternalRef::new("git", "commit", "candidate")
                .unwrap_or_else(|_| unreachable!()),
            tree: ExternalRef::new("git", "tree", "tree").unwrap_or_else(|_| unreachable!()),
            changed_paths: vec!["crates/src/lib.rs".to_string()],
            acceptance_digest: super::super::acceptance_digest(work)
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

    fn make_receipt(
        work: &ImplementationWork,
        handoff: &CandidateHandoff,
        handoff_identity: &ContentId,
    ) -> CandidateVerification {
        CandidateVerification {
            schema: VERIFICATION_SCHEMA.to_string(),
            handoff: handoff_identity.clone(),
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

    #[test]
    // r[verify kiln.implementation.verification.scenario.admit]
    fn admits_and_retains_canonical_verification_once() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .reserve_dispatch(
                &attempt.identity,
                attempt.fence,
                1,
                &content('d'),
                &crate::types::ProviderProfileId::new("lattice").unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        ledger
            .start_execution(
                &attempt.identity,
                attempt.fence,
                2,
                &ExternalRef::new("lattice", "run", "one").unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        let handoff = handoff(&work, &attempt);
        let handoff_identity = ledger
            .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 3, &handoff)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        ledger
            .begin_verification(&attempt.identity, attempt.fence, 4)
            .unwrap_or_else(|_| unreachable!());
        let receipt = make_receipt(&work, &handoff, &handoff_identity);
        let identity = ledger
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence,
                5,
                &handoff,
                &receipt,
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            ledger
                .record_verification(
                    &work,
                    &attempt.identity,
                    attempt.fence,
                    6,
                    &handoff,
                    &receipt
                )
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            identity
        );
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.phase, AttemptPhase::Verified);
        assert!(stored.verification.is_some());
        assert!(ledger.current(&attempt.canonical_key).is_none());
    }

    #[test]
    // r[verify kiln.implementation.verification.scenario.refused]
    fn refuses_failed_partial_or_mismatched_evidence() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let handoff = handoff(&work, &attempt);
        let handoff_identity = super::super::admit_candidate_handoff(&work, &attempt, &handoff)
            .unwrap_or_else(|_| unreachable!());
        let mut receipt = make_receipt(&work, &handoff, &handoff_identity);
        receipt.run.success = false;
        assert!(admit_candidate_verification(&work, &attempt, &handoff, &receipt).is_err());
        receipt.run.success = true;
        receipt.run.checks.clear();
        assert!(admit_candidate_verification(&work, &attempt, &handoff, &receipt).is_err());
        receipt = make_receipt(&work, &handoff, &handoff_identity);
        receipt
            .run
            .checks
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .outcome = "fail".to_string();
        assert!(admit_candidate_verification(&work, &attempt, &handoff, &receipt).is_err());
        let mut wrong = make_receipt(&work, &handoff, &handoff_identity);
        wrong.run.revision = "b".repeat(40);
        assert!(admit_candidate_verification(&work, &attempt, &handoff, &wrong).is_err());
    }
}
