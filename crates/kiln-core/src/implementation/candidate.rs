//! Bounded candidate handoff admission for one implementation attempt.
//!
//! The handoff binds the exact work/version, attempt/fence, repository/base,
//! candidate and tree, changed paths, acceptance-obligation digest,
//! verification profile, toolchain/environment identities and retained
//! evidence references. Admission proves only that the envelope is complete and
//! consistent. It does not execute checks, accept a candidate or publish work.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{ImplementationWork, content_identity, push_str};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const CANDIDATE_HANDOFF_SCHEMA: &str = "kiln.implementation-candidate-handoff.v1";
pub const MAX_CHANGED_PATHS: usize = 4_096;
pub const MAX_CANDIDATE_EVIDENCE: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Check,
    Artifact,
    Reproducer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Passed,
    Failed,
    Unknown,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateEvidence {
    pub identity: ContentId,
    pub kind: EvidenceKind,
    pub outcome: EvidenceOutcome,
    pub reference: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateHandoff {
    pub schema: String,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt_identity: ContentId,
    pub fence: u64,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<ExternalRef>,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub changed_paths: Vec<String>,
    pub acceptance_digest: ContentId,
    pub verification_profile: ContentId,
    pub toolchain: ContentId,
    pub environment: ContentId,
    pub evidence: Vec<CandidateEvidence>,
}

/// Canonical digest of the work's reviewed acceptance obligations.
///
/// A candidate for another work version or another obligation set has a
/// different digest, so a valid verification result cannot be transferred.
// r[impl kiln.implementation.candidate_handoff]
pub fn acceptance_digest(work: &ImplementationWork) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_str(&mut bytes, super::IMPLEMENTATION_WORK_SCHEMA)?;
    push_str(&mut bytes, work.identity()?.as_str())?;
    let mut obligations: Vec<&super::AcceptanceObligation> = work.acceptance.iter().collect();
    obligations.sort_by(|left, right| left.id.cmp(&right.id));
    for obligation in obligations {
        push_str(&mut bytes, &obligation.id)?;
        push_str(&mut bytes, &obligation.statement)?;
    }
    content_identity(&bytes)
}

/// Admit one candidate handoff against the exact work and attempt.
///
/// Returns the canonical handoff identity. Admission does not execute or accept
/// the candidate; retained evidence outcomes remain observations.
// r[impl kiln.implementation.candidate_handoff]
pub fn admit_candidate_handoff(
    work: &ImplementationWork,
    attempt: &super::ImplementationAttempt,
    handoff: &CandidateHandoff,
) -> CoreResult<ContentId> {
    if attempt
        .handoff
        .as_ref()
        .is_some_and(|stored| stored != handoff)
        || attempt
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate != &handoff.candidate)
        || handoff.schema != CANDIDATE_HANDOFF_SCHEMA
        || handoff.work_identity != work.identity()?
        || handoff.work_revision != work.revision
        || handoff.work_identity != attempt.work_identity
        || handoff.work_revision != attempt.work_revision
        || handoff.attempt_identity != attempt.identity
        || handoff.fence != attempt.fence
        || handoff.repository != work.repository
        || handoff.base != work.base
        || handoff.base != attempt.base
        || handoff.acceptance_digest != acceptance_digest(work)?
        || handoff.verification_profile != work.verification_profile
    {
        return Err(refusal(
            "implementation_candidate_binding",
            "candidate.handoff",
            "candidate handoff does not bind the exact work, attempt and acceptance contract",
        ));
    }
    handoff.candidate.validate()?;
    handoff.tree.validate()?;
    if !valid_hex(&handoff.acceptance_digest)
        || !valid_hex(&handoff.verification_profile)
        || !valid_hex(&handoff.toolchain)
        || !valid_hex(&handoff.environment)
        || !handoff
            .evidence
            .iter()
            .all(|item| valid_hex(&item.identity))
        || handoff.candidate == handoff.tree
        || handoff.changed_paths.is_empty()
        || handoff.changed_paths.len() > MAX_CHANGED_PATHS
        || handoff.evidence.is_empty()
        || handoff.evidence.len() > MAX_CANDIDATE_EVIDENCE
    {
        return Err(refusal(
            "implementation_candidate_bounds",
            "candidate.handoff",
            "candidate identity, changed paths or evidence are empty or over bound",
        ));
    }
    let admitted_paths: Vec<&str> = work.permitted_paths.iter().map(String::as_str).collect();
    let mut changed = BTreeSet::new();
    for path in &handoff.changed_paths {
        if !safe_relative(path)
            || !changed.insert(path.as_str())
            || !admitted_paths
                .iter()
                .any(|permitted| under_path(path, permitted))
        {
            return Err(refusal(
                "implementation_candidate_paths",
                "candidate.changed_paths",
                "candidate paths are malformed, duplicated or outside the admitted work",
            ));
        }
    }
    let mut evidence = BTreeSet::new();
    for item in &handoff.evidence {
        item.reference.validate()?;
        if !evidence.insert(&item.identity) {
            return Err(refusal(
                "implementation_candidate_evidence",
                "candidate.evidence",
                "candidate evidence identity is duplicated",
            ));
        }
    }
    let mut bytes = Vec::new();
    push_str(&mut bytes, CANDIDATE_HANDOFF_SCHEMA)?;
    push_str(&mut bytes, handoff.work_identity.as_str())?;
    bytes.extend_from_slice(&handoff.work_revision.to_be_bytes());
    push_str(&mut bytes, handoff.attempt_identity.as_str())?;
    bytes.extend_from_slice(&handoff.fence.to_be_bytes());
    push_str(&mut bytes, &handoff.repository)?;
    push_external(&mut bytes, handoff.base.as_ref())?;
    push_external(&mut bytes, Some(&handoff.candidate))?;
    push_external(&mut bytes, Some(&handoff.tree))?;
    let mut paths: Vec<&str> = handoff.changed_paths.iter().map(String::as_str).collect();
    paths.sort_unstable();
    for path in paths {
        push_str(&mut bytes, path)?;
    }
    push_str(&mut bytes, handoff.acceptance_digest.as_str())?;
    push_str(&mut bytes, handoff.verification_profile.as_str())?;
    push_str(&mut bytes, handoff.toolchain.as_str())?;
    push_str(&mut bytes, handoff.environment.as_str())?;
    let mut evidence: Vec<&CandidateEvidence> = handoff.evidence.iter().collect();
    evidence.sort_by(|left, right| {
        left.identity
            .cmp(&right.identity)
            .then_with(|| evidence_kind(left.kind).cmp(evidence_kind(right.kind)))
            .then_with(|| evidence_outcome(left.outcome).cmp(evidence_outcome(right.outcome)))
            .then_with(|| left.reference.cmp(&right.reference))
    });
    for item in evidence {
        push_str(&mut bytes, item.identity.as_str())?;
        push_str(&mut bytes, evidence_kind(item.kind))?;
        push_str(&mut bytes, evidence_outcome(item.outcome))?;
        push_external(&mut bytes, Some(&item.reference))?;
    }
    content_identity(&bytes)
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn under_path(path: &str, permitted: &str) -> bool {
    path == permitted
        || path
            .strip_prefix(permitted)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn evidence_kind(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::Check => "check",
        EvidenceKind::Artifact => "artifact",
        EvidenceKind::Reproducer => "reproducer",
    }
}

fn evidence_outcome(outcome: EvidenceOutcome) -> &'static str {
    match outcome {
        EvidenceOutcome::Passed => "passed",
        EvidenceOutcome::Failed => "failed",
        EvidenceOutcome::Unknown => "unknown",
        EvidenceOutcome::Unavailable => "unavailable",
    }
}

fn valid_hex(identity: &ContentId) -> bool {
    identity.as_str().len() == 64
        && identity
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
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

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::{
        AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use alloc::collections::BTreeSet;
    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn reference(kind: &str, id: &str) -> ExternalRef {
        ExternalRef::new("git", kind, id).unwrap_or_else(|_| unreachable!())
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: None,
            title: "Candidate work".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates/kiln-core".to_string()],
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
            schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: None,
            candidate: reference("commit", "candidate"),
            tree: reference("tree", "tree"),
            changed_paths: vec!["crates/kiln-core/src/lib.rs".to_string()],
            acceptance_digest: acceptance_digest(work).unwrap_or_else(|_| unreachable!()),
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

    fn executing(
        work: &ImplementationWork,
    ) -> (AttemptLedger, super::super::ImplementationAttempt) {
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(work, 0, DEFAULT_LEASE_MS)
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
        (ledger, attempt)
    }

    #[test]
    // r[verify kiln.implementation.candidate_handoff] r[verify kiln.implementation.candidate_handoff.scenario.admit]
    fn admits_and_retains_one_exact_candidate_handoff() {
        let work = work();
        let (mut ledger, attempt) = executing(&work);
        let handoff = handoff(&work, &attempt);
        let digest = admit_candidate_handoff(&work, &attempt, &handoff)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let stored = ledger
            .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 3, &handoff)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(stored, digest);
        assert_eq!(
            ledger
                .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 4, &handoff)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            digest
        );
        assert_eq!(
            ledger
                .candidate_handoff(&attempt.identity)
                .cloned()
                .unwrap_or_else(|| unreachable!()),
            handoff
        );
        let attempt = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(attempt.phase, super::super::AttemptPhase::CandidateProduced);
        let mut forged = handoff.clone();
        forged.candidate = reference("commit", "other");
        assert!(
            admit_candidate_handoff(&work, attempt, &forged).is_err(),
            "a substituted candidate cannot reuse the retained handoff"
        );
    }

    #[test]
    // r[verify kiln.implementation.candidate_handoff.scenario.refused]
    fn refuses_mismatched_or_broader_handoffs() {
        let work = work();
        let (mut ledger, attempt) = executing(&work);
        let good = handoff(&work, &attempt);
        let cases: [(&str, CandidateHandoff); 7] = [
            ("binding", {
                let mut handoff = good.clone();
                handoff.work_revision = 2;
                handoff
            }),
            ("binding", {
                let mut handoff = good.clone();
                handoff.repository = "other/repo".to_string();
                handoff
            }),
            ("binding", {
                let mut handoff = good.clone();
                handoff.acceptance_digest = content('a');
                handoff
            }),
            ("paths", {
                let mut handoff = good.clone();
                handoff.changed_paths = vec!["/etc/passwd".to_string()];
                handoff
            }),
            ("paths", {
                let mut handoff = good.clone();
                handoff.changed_paths = vec!["crates/other/src/lib.rs".to_string()];
                handoff
            }),
            ("evidence", {
                let mut handoff = good.clone();
                let mut duplicate = handoff
                    .evidence
                    .first()
                    .cloned()
                    .unwrap_or_else(|| unreachable!());
                duplicate.reference =
                    ExternalRef::new("kiln", "run", "other").unwrap_or_else(|_| unreachable!());
                handoff.evidence.push(duplicate);
                handoff
            }),
            ("bounds", {
                let mut handoff = good.clone();
                handoff.tree = handoff.candidate.clone();
                handoff
            }),
        ];
        for (kind, handoff) in cases {
            let error = admit_candidate_handoff(&work, &attempt, &handoff)
                .err()
                .unwrap_or_else(|| unreachable!("{kind}"));
            assert!(
                error.code.as_str() == format!("implementation_candidate_{kind}"),
                "{error:?}"
            );
        }
        // The free admission does not mutate the ledger.
        assert!(ledger.candidate_handoff(&attempt.identity).is_none());
        // A candidate cannot be handed off before the attempt executes.
        let handoff = handoff(&work, &attempt);
        assert!(
            ledger
                .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 3, &handoff)
                .is_ok()
        );
    }
}
