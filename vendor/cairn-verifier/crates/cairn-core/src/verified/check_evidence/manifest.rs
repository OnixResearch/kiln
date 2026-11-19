//! Consumer-owned final-manifest admission over bounded, already loaded bytes.
use super::*;
use crate::{canonical_json_hash, evaluate_authoritative_agent_completion};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BUNDLE_ARTIFACTS: usize = MAX_CHECKS + 3;
pub const MAX_BUNDLE_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDecision {
    Complete,
    InspectAndUseNewOutputRoot,
}

/// No observed interrupted state authorizes retry, deletion, or finalization.
pub fn recovery_decision(manifest_admitted: bool) -> RecoveryDecision {
    if manifest_admitted {
        RecoveryDecision::Complete
    } else {
        RecoveryDecision::InspectAndUseNewOutputRoot
    }
}

// r[impl cairn.check_evidence_producer.publication]
pub fn admit_manifest(
    plan: &CheckEvidencePlan,
    attempt: &str,
    manifest: &Value,
    artifacts: &BTreeMap<String, Vec<u8>>,
) -> Result<(), ProducerIssue> {
    let reject = ProducerIssue::PublicationIncomplete;
    if !super::types::digest(attempt)
        || manifest["schema"] != "cairn.check-evidence-run.v1"
        || manifest["valid"] != true
        || manifest["complete"] != true
        || manifest["plan_hash"] != plan.identity()
        || manifest["attempt_hash"] != attempt
        || manifest["blockers"] != json!([])
        || manifest["non_claims"] != json!(NON_CLAIMS)
        || manifest["source_scope"] != "explicit_file_projection"
        || artifacts.len() > MAX_BUNDLE_ARTIFACTS
    {
        return Err(reject);
    }
    let declared = manifest["artifacts"].as_object().ok_or(reject)?;
    let mut expected: BTreeSet<String> = ["attempt.json", "observations.json", "results.json"]
        .into_iter()
        .map(String::from)
        .collect();
    expected.extend(
        plan.profiles()
            .iter()
            .map(|p| format!("{}.facts.json", p.check_id)),
    );
    if declared.keys().cloned().collect::<BTreeSet<_>>() != expected
        || artifacts.keys().cloned().collect::<BTreeSet<_>>() != expected
    {
        return Err(reject);
    }
    let mut decoded = BTreeMap::new();
    let mut total_bytes = 0_usize;
    for (leaf, bytes) in artifacts {
        total_bytes = total_bytes.checked_add(bytes.len()).ok_or(reject)?;
        if bytes.len() > MAX_BUNDLE_ARTIFACT_BYTES
            || total_bytes > MAX_BUNDLE_BYTES
            || declared[leaf] != blake3::hash(bytes).to_hex().as_str()
        {
            return Err(reject);
        }
        let value: Value = serde_json::from_slice(bytes).map_err(|_| reject)?;
        decoded.insert(leaf.as_str(), value);
    }
    if decoded["attempt.json"]
        != json!({"attempt_hash":attempt,"plan_hash":plan.identity(),"state":"started"})
        || decoded["results.json"] != manifest["results"]
    {
        return Err(reject);
    }
    let results = manifest["results"].as_array().ok_or(reject)?;
    let observations = decoded["observations.json"].as_array().ok_or(reject)?;
    if results.len() != plan.profiles().len() || observations.len() != results.len() {
        return Err(reject);
    }
    let completion = evaluate_authoritative_agent_completion(
        &json!(plan.contract),
        &manifest["results"],
        &plan.applicability.subject_hash,
        &plan.applicability.policy_hash,
    );
    if !completion.valid {
        return Err(reject);
    }
    for ((profile, row), observation) in plan.profiles().iter().zip(results).zip(observations) {
        let leaf = format!("{}.facts.json", profile.check_id);
        let facts = &decoded[leaf.as_str()];
        if row["check_id"] != profile.check_id
            || observation["check_id"] != profile.check_id
            || observation["attempt_hash"] != attempt
            || observation["plan_hash"] != plan.identity()
            || observation["applicability"] != json!(plan.applicability())
            || observation["source_after_hash"] != plan.applicability().subject_hash
            || observation["executable_hash"] != profile.executable_hash
            || observation["tool_cohort_hash"] != profile.tool_cohort_hash
            || observation["command_hash"] != row["command_hash"]
            || observation["outcome"] != "exited_zero"
            || observation["native_status"] != "passed"
            || observation["execution_observed"] != true
            || observation["teardown_complete"] != true
            || observation["cache_claimed"] != false
            || observation["payload_passed"] != true
            || observation["producer_identity"]
                != blake3::hash(LOCAL_EXECUTOR_REVISION.as_bytes())
                    .to_hex()
                    .as_str()
            || observation["native_non_claims"]
                .as_array()
                .is_none_or(|claims| {
                    claims.is_empty()
                        || claims.len() > MAX_CHECKS
                        || claims.iter().any(|claim| {
                            claim
                                .as_str()
                                .is_none_or(|text| text.is_empty() || text.len() > MAX_TEXT_BYTES)
                        })
                })
            || facts["producer_revision"] != LOCAL_EXECUTOR_REVISION
            || !local_payload_matches(profile, observation, facts)
            || facts["schema"] != "cairn.local-check-facts.v1"
            || facts["attempt_hash"] != attempt
            || facts["plan_hash"] != plan.identity()
            || facts["check_id"] != profile.check_id
            || row["evidence_hash"] != canonical_json_hash(facts)
            || observation["evidence_hash"] != row["evidence_hash"]
            || row["execution_receipt_hash"]
                != canonical_json_hash(
                    &json!({"schema":"cairn.check-observation.v1", "observation":observation,"non_claims":NON_CLAIMS}),
                )
        {
            return Err(reject);
        }
    }
    Ok(())
}

fn local_payload_matches(profile: &CheckProfile, observation: &Value, facts: &Value) -> bool {
    // Only Cargo has an admitted local payload grammar. Nix remains process-only.
    profile.kind == CheckKind::CargoTest
        && observation["role"] == "test"
        && observation["executed_tests"]
            .as_u64()
            .is_some_and(|count| count > 0)
        && facts["executed_tests"] == observation["executed_tests"]
        && facts["payload_passed"] == true
        && facts["facts"]["outcome"] == "exited_zero"
        && facts["facts"]["stdout_truncated"] == false
        && facts["facts"]["stderr_truncated"] == false
}
