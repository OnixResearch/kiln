use std::collections::BTreeSet;

use serde_json::json;

use crate::verified::agent_receipt::{
    authoritative_command_hash, evaluate_authoritative_agent_completion, AgentVerificationContract,
};
use crate::verified::evidence::canonical_json_hash;

use super::types::*;

// r[impl cairn.check_evidence_producer.contract]
pub fn admit_plan(
    contract: &AgentVerificationContract,
    profiles: &[CheckProfile],
    applicability: &Applicability,
) -> Result<CheckEvidencePlan, ProducerIssue> {
    if contract.required_checks.is_empty()
        || contract.required_checks.len() > MAX_CHECKS
        || profiles.len() != contract.required_checks.len()
        || applicability.dependency_hashes.len() > MAX_CHECKS
    {
        return Err(ProducerIssue::Bounds);
    }
    if contract.repository_id.len() > MAX_TEXT_BYTES
        || contract.registry_id.len() > MAX_TEXT_BYTES
        || contract.required_checks.iter().any(|c| {
            !label(&c.check_id)
                || c.command.len() > MAX_FEATURES
                || c.command.iter().any(|s| s.len() > MAX_TEXT_BYTES)
                || c.prerequisites.len() > MAX_CHECKS
                || c.prerequisites.iter().any(|s| !label(s))
        })
    {
        return Err(ProducerIssue::Bounds);
    }
    if !valid_applicability(applicability) {
        return Err(ProducerIssue::Applicability);
    }
    if contract.schema != "cairn.agent-completion.v2"
        || !digest(&contract.registry_hash)
        || !digest(&contract.selection_hash)
        || !digest(&contract.policy_hash)
        || !digest(&contract.final_subject_hash)
        || contract
            .required_checks
            .iter()
            .any(|c| !digest(&c.command_hash))
    {
        return Err(ProducerIssue::Contract);
    }
    // Reuse the completion owner. Missing results are expected at planning time.
    let report = evaluate_authoritative_agent_completion(
        &json!(contract),
        &json!([]),
        &applicability.subject_hash,
        &applicability.policy_hash,
    );
    if report
        .diagnostics
        .iter()
        .any(|d| d.code.starts_with("verification_contract."))
    {
        return Err(ProducerIssue::Contract);
    }
    let mut seen = BTreeSet::new();
    for profile in profiles {
        validate_profile(profile)?;
        if !seen.insert(&profile.check_id) {
            return Err(ProducerIssue::Selection);
        }
        let check = contract
            .required_checks
            .iter()
            .find(|c| c.check_id == profile.check_id)
            .ok_or(ProducerIssue::Selection)?;
        if check.command != profile_command(profile)
            || check.prerequisites.len() > MAX_CHECKS
            || check.prerequisites.iter().collect::<BTreeSet<_>>().len()
                != check.prerequisites.len()
        {
            return Err(ProducerIssue::Command);
        }
    }
    // Stable dependency-first order, independent of producer row order.
    let mut ordered = Vec::with_capacity(profiles.len());
    let mut completed = BTreeSet::new();
    while ordered.len() < profiles.len() {
        let next = profiles
            .iter()
            .filter(|p| !completed.contains(&p.check_id))
            .filter(|p| {
                contract.required_checks.iter().any(|c| {
                    c.check_id == p.check_id
                        && c.prerequisites.iter().all(|id| completed.contains(id))
                })
            })
            .min_by_key(|p| &p.check_id)
            .ok_or(ProducerIssue::Contract)?;
        completed.insert(next.check_id.clone());
        ordered.push(next.clone());
    }
    let mut contract = contract.clone();
    contract
        .required_checks
        .sort_by(|a, b| a.check_id.cmp(&b.check_id));
    for check in &mut contract.required_checks {
        check.prerequisites.sort();
    }
    let mut plan = CheckEvidencePlan {
        schema: PLAN_SCHEMA.into(),
        contract,
        applicability: applicability.clone(),
        profiles: ordered,
        plan_hash: String::new(),
        non_claims: NON_CLAIMS.iter().map(|s| (*s).into()).collect(),
    };
    plan.plan_hash = canonical_json_hash(&json!(plan));
    Ok(plan)
}

// r[impl cairn.check_evidence_producer.applicability]
// r[impl cairn.verification_impact.applicability]
pub fn valid_applicability(a: &Applicability) -> bool {
    digest(&a.subject_hash)
        && digest(&a.policy_hash)
        && digest(&a.source_projection_hash)
        && a.dependency_hashes.iter().all(|h| digest(h))
        && canonical(&a.dependency_hashes)
}

fn validate_profile(p: &CheckProfile) -> Result<(), ProducerIssue> {
    let b = &p.bounds;
    if b.deadline_ms == 0
        || b.deadline_ms > MAX_DEADLINE_MS
        || b.teardown_ms == 0
        || b.teardown_ms > MAX_TEARDOWN_MS
        || b.stdout_bytes == 0
        || b.stdout_bytes > MAX_STREAM_BYTES
        || b.stderr_bytes == 0
        || b.stderr_bytes > MAX_STREAM_BYTES
        || p.features.len() > MAX_FEATURES
        || p.environment.len() > MAX_ENVIRONMENT
    {
        return Err(ProducerIssue::Bounds);
    }
    if p.schema != PROFILE_SCHEMA
        || !label(&p.check_id)
        || !label(&b.id)
        || !label(&p.target)
        || !digest(&p.executable_hash)
        || !digest(&p.tool_cohort_hash)
        || p.working_root != "."
        || !canonical(&p.features)
        || !p.features.iter().all(|s| label(s))
        || !valid_executable(&p.executable, p.kind)
        || (p.kind == CheckKind::NixCheck && !p.features.is_empty())
    {
        return Err(ProducerIssue::Profile);
    }
    // Explicit values only. Environment identity is included in the plan.
    // This admission does not claim that an environment is safe or sandboxed.
    if p.environment.iter().any(|(k, v)| {
        k.is_empty()
            || k.len() > 128
            || !k
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            || v.len() > MAX_TEXT_BYTES
            || v.bytes().any(|b| b < 32 || b == 127)
    }) {
        return Err(ProducerIssue::Profile);
    }
    Ok(())
}

fn valid_executable(value: &str, kind: CheckKind) -> bool {
    let suffix = match kind {
        CheckKind::CargoTest => "/bin/cargo",
        CheckKind::NixCheck => "/bin/nix",
    };
    value.len() <= MAX_TEXT_BYTES
        && value.starts_with("/nix/store/")
        && value.ends_with(suffix)
        && value
            .split('/')
            .skip(1)
            .all(|s| !s.is_empty() && s != "." && s != "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._+-".contains(&b))
}

/// Fixed grammars have no arbitrary trailing arguments or shell interpolation.
pub fn profile_command(p: &CheckProfile) -> Vec<String> {
    match p.kind {
        CheckKind::CargoTest => {
            let mut argv = vec![
                p.executable.clone(),
                "test".into(),
                "--workspace".into(),
                "--all-targets".into(),
                "--locked".into(),
                "--offline".into(),
                "--target".into(),
                p.target.clone(),
                "--no-default-features".into(),
            ];
            if !p.features.is_empty() {
                argv.extend(["--features".into(), p.features.join(",")]);
            }
            argv
        }
        // The Nix grammar evaluates a fixture-owned expression file. A real
        // `nix` process runs with a fixed argv, but evaluation needs no store
        // daemon or writable store, so the same profile works in a development
        // shell and inside a sandboxed Nix check derivation.
        CheckKind::NixCheck => vec![
            p.executable.clone(),
            "eval".into(),
            "--offline".into(),
            "--json".into(),
            "--file".into(),
            "./check.nix".into(),
        ],
    }
}

/// Authorization facts come from the shell, never from a saved producer payload.
// r[impl cairn.check_evidence_producer.local_run]
pub fn authorize_attempt(
    plan: &CheckEvidencePlan,
    check_id: &str,
    execute: bool,
    reviewed_plan_hash: &str,
    observed_executable_hash: &str,
    completed: &[NormalizedAttempt],
) -> Result<(), ProducerIssue> {
    if !execute {
        return Err(ProducerIssue::Denied);
    }
    if completed.len() > MAX_CHECKS {
        return Err(ProducerIssue::Bounds);
    }
    if reviewed_plan_hash != plan.identity() {
        return Err(ProducerIssue::PlanChanged);
    }
    let profile = plan
        .profiles
        .iter()
        .find(|p| p.check_id == check_id)
        .ok_or(ProducerIssue::Selection)?;
    if observed_executable_hash != profile.executable_hash {
        return Err(ProducerIssue::ExecutableChanged);
    }
    let check = plan
        .contract
        .required_checks
        .iter()
        .find(|c| c.check_id == check_id)
        .ok_or(ProducerIssue::Selection)?;
    if check.command_hash != authoritative_command_hash(&profile_command(profile)) {
        return Err(ProducerIssue::Command);
    }
    let mut seen = BTreeSet::new();
    if completed.iter().any(|a| {
        a.observation.plan_hash != plan.plan_hash
            || a.result.check_id == check_id
            || !seen.insert(&a.result.check_id)
    }) {
        return Err(ProducerIssue::Prerequisite);
    }
    if check.prerequisites.iter().any(|id| {
        completed
            .iter()
            .filter(|a| {
                a.result.check_id == *id
                    && a.result.status == "passed"
                    && a.observation.plan_hash == plan.plan_hash
            })
            .count()
            != 1
    }) {
        return Err(ProducerIssue::Prerequisite);
    }
    Ok(())
}
