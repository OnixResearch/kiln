use std::collections::BTreeSet;

use serde_json::json;

use crate::verified::agent_receipt::{authoritative_command_hash, AgentExecutionResult};
use crate::verified::evidence::canonical_json_hash;

use super::types::*;

// r[impl cairn.check_evidence_producer.applicability]
pub fn normalize_attempt(
    plan: &CheckEvidencePlan,
    observation: &AttemptObservation,
) -> Result<NormalizedAttempt, ProducerIssue> {
    let p = plan
        .profiles
        .iter()
        .find(|p| p.check_id == observation.check_id)
        .ok_or(ProducerIssue::Selection)?;
    if observation.plan_hash != plan.plan_hash
        || observation.applicability != plan.applicability
        || observation.source_after_hash != plan.applicability.subject_hash
        || observation.executable_hash != p.executable_hash
        || observation.tool_cohort_hash != p.tool_cohort_hash
        || observation.command_hash != authoritative_command_hash(&super::profile_command(p))
    {
        return Err(ProducerIssue::Applicability);
    }
    if !digest(&observation.attempt_hash)
        || !digest(&observation.evidence_hash)
        || !digest(&observation.producer_identity)
        || !label(&observation.native_status)
        || observation.native_non_claims.len() > MAX_CHECKS
        || observation.native_non_claims.iter().any(|s| {
            s.is_empty() || s.len() > MAX_TEXT_BYTES || s.bytes().any(|b| b < 32 || b == 127)
        })
    {
        return Err(ProducerIssue::Observation);
    }
    let status = classify(p, observation);
    let result = AgentExecutionResult {
        check_id: p.check_id.clone(),
        status: status.into(),
        command: super::profile_command(p),
        command_hash: observation.command_hash.clone(),
        subject_hash: plan.applicability.subject_hash.clone(),
        registry_hash: plan.contract.registry_hash.clone(),
        selection_hash: plan.contract.selection_hash.clone(),
        policy_hash: plan.applicability.policy_hash.clone(),
        evidence_hash: observation.evidence_hash.clone(),
        execution_receipt_hash: canonical_json_hash(&json!({
            "schema": "cairn.check-observation.v1", "observation": observation, "non_claims": NON_CLAIMS,
        })),
        artifacts: Vec::new(),
        diagnostic: None,
    };
    Ok(NormalizedAttempt {
        observation: observation.clone(),
        result,
    })
}

fn classify(profile: &CheckProfile, o: &AttemptObservation) -> &'static str {
    // Unknown cleanup dominates a nominally successful child exit.
    if !o.teardown_complete || o.outcome == ProcessOutcome::TeardownFailed {
        return "cleanup_failed";
    }
    match o.outcome {
        ProcessOutcome::Denied => "blocked",
        ProcessOutcome::TimedOut => "timed_out",
        ProcessOutcome::Cancelled => "cancelled",
        ProcessOutcome::OutputOverflow | ProcessOutcome::InfrastructureError => {
            "infrastructure_error"
        }
        ProcessOutcome::ExitedNonzero => "failed",
        ProcessOutcome::Unknown => "unknown",
        ProcessOutcome::TeardownFailed => "cleanup_failed",
        ProcessOutcome::ExitedZero => classify_payload(profile, o),
    }
}

fn classify_payload(profile: &CheckProfile, o: &AttemptObservation) -> &'static str {
    if !o.execution_observed || o.cache_claimed {
        return "unknown";
    }
    if !o.payload_passed {
        return "failed";
    }
    // This vocabulary belongs to the local observation port, not an upstream decoder.
    // Saved producers require their own pinned admission before they can use this port.
    if o.native_status != "passed" {
        return "blocked";
    }
    let role_matches = match profile.kind {
        CheckKind::CargoTest => o.role == EvidenceRole::Test && o.executed_tests > 0,
        // A Nix check is a build obligation, not a test-coverage claim.
        CheckKind::NixCheck => o.role == EvidenceRole::Build,
    };
    if role_matches {
        "passed"
    } else {
        "blocked"
    }
}

// r[impl cairn.check_evidence_producer.applicability]
// r[impl cairn.verification_impact.applicability]
/// Single home for whole-subject evidence reuse: recorded evidence stays
/// current only under exact applicability equality with the current state.
/// Source changes, policy changes, dependency drift, and projection changes
/// each force staleness; missing marker links never preserve evidence.
pub fn whole_subject_evidence_current(recorded: &Applicability, current: &Applicability) -> bool {
    recorded == current
}

// r[impl cairn.check_evidence_producer.publication]
pub fn publication_decision(
    plan: &CheckEvidencePlan,
    attempts: &[NormalizedAttempt],
    artifacts: &[PublishedArtifact],
    current: &Applicability,
) -> PublicationDecision {
    if !whole_subject_evidence_current(&plan.applicability, current)
        || attempts.len() != plan.profiles.len()
        || artifacts.len() != attempts.len()
        || artifacts.len() > MAX_CHECKS
    {
        return PublicationDecision::Withhold;
    }
    let mut seen = BTreeSet::new();
    for ((attempt, artifact), profile) in attempts.iter().zip(artifacts).zip(&plan.profiles) {
        if attempt.result.check_id != profile.check_id
            || attempt.observation.plan_hash != plan.plan_hash
            || attempts.first().is_none_or(|first| {
                first.observation.attempt_hash != attempt.observation.attempt_hash
            })
            || attempt.result.status != "passed"
            || !seen.insert(&attempt.result.check_id)
            || !plan
                .profiles
                .iter()
                .any(|p| p.check_id == attempt.result.check_id)
            || !artifact.durable
            || artifact.expected_hash != artifact.observed_hash
            || artifact.expected_hash != attempt.result.evidence_hash
        {
            return PublicationDecision::Withhold;
        }
    }
    PublicationDecision::PublishFinalManifest
}
