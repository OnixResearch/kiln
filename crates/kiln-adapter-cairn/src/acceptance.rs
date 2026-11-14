//! Authoritative Cairn acceptance of independently executed Kiln observations.
//! This adapter does not execute checks or perform lifecycle effects.
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;

use cairn_core::{
    AgentArtifactReference, AgentExecutionResult, AgentVerificationContract, canonical_json_hash,
    evaluate_authoritative_agent_completion,
};
use kiln_core::change_review::CiCheck;
use kiln_core::implementation::{
    CandidateHandoff, CandidateVerification, ImplementationAttempt, ImplementationWork,
    VERIFICATION_SCHEMA, WorkSourceKind, admit_candidate_handoff, admit_candidate_verification,
};
use kiln_implementer::verification::{FrozenProgram, VerificationRun, command};

/// Adapt only controller-created observations. A JSON worker manifest cannot
/// construct VerificationRun and CLI schema validation cannot substitute for it.
// r[impl kiln.implementation.verification]
pub fn verify_cairn_completion(
    executed: &VerificationRun,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
) -> Result<CandidateVerification, String> {
    executed.assert_binding(work, attempt, handoff)?;
    let profile = executed.profile();
    let cairn = profile
        .cairn
        .as_ref()
        .ok_or("missing admitted Cairn verification contract")?;
    if cairn.aliases != work.aliases
        || !work
            .aliases
            .iter()
            .any(|alias| alias.kind == WorkSourceKind::CairnTask)
        || !executed.checks_executed()
        || !executed.run().success
    {
        return Err("Cairn outer work/task/spec or executed verification is ineligible".into());
    }
    let mut contract: AgentVerificationContract =
        serde_json::from_value(cairn.contract.clone()).map_err(text)?;
    let registry = frozen_json(&cairn.registry)?;
    let policy = frozen_json(&cairn.policy)?;
    if contract.repository_id != work.repository
        || !contract.final_subject_hash.is_empty()
        || contract.registry_hash != canonical_json_hash(&registry)
        || contract.policy_hash != canonical_json_hash(&policy)
    {
        return Err(
            "Cairn repository/current registry/current policy or subject template differs".into(),
        );
    }
    // The selection was frozen before editing. Bind it to the exact profile
    // commands/prerequisites, never to a worker's modified task/check list.
    let selected: BTreeSet<_> = contract
        .required_checks
        .iter()
        .map(|check| check.check_id.as_str())
        .collect();
    let expected: BTreeSet<_> = profile
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    if selected != expected || selected.len() != contract.required_checks.len() {
        return Err("Cairn required checks differ from admitted executed profile".into());
    }
    for required in &contract.required_checks {
        let check = profile
            .checks
            .iter()
            .find(|check| check.id == required.check_id)
            .ok_or("missing frozen check")?;
        if required.command != command(check) || required.prerequisites != check.prerequisites {
            return Err("Cairn required command/prerequisites differ from frozen execution".into());
        }
    }
    contract.final_subject_hash = executed.subject_hash().into();
    let results: Vec<_> = executed
        .observations()
        .iter()
        .filter(|row| !row.baseline)
        .map(|row| adapt_observation(row, &contract))
        .collect();
    let report = evaluate_authoritative_agent_completion(
        &serde_json::to_value(&contract).map_err(text)?,
        &serde_json::to_value(&results).map_err(text)?,
        executed.subject_hash(),
        &canonical_json_hash(&policy),
    );
    // Retain the producer report, even when completion remains unmet.
    fs::write(
        executed.evidence_directory().join("cairn-completion.json"),
        serde_json::to_vec_pretty(&report).map_err(text)?,
    )
    .map_err(text)?;
    let mut material = serde_json::to_value(&report).map_err(text)?;
    material
        .as_object_mut()
        .ok_or("native completion report is not an object")?
        .remove("receipt_hash");
    fs::write(
        executed
            .evidence_directory()
            .join("cairn-completion-material.json"),
        cairn_core::canonical_json(&material).as_bytes(),
    )
    .map_err(text)?;
    if !report.valid {
        return Err(format!(
            "Cairn authoritative completion unmet: {}",
            report.receipt_hash
        ));
    }
    let mut run = executed.run().clone();
    if run
        .checks
        .iter()
        .any(|check| check.name == "cairn-authoritative-completion")
    {
        return Err("reserved completion check name collides with profile".into());
    }
    run.checks.push(CiCheck {
        name: "cairn-authoritative-completion".into(),
        kind: "cairn-agent-completion-v2".into(),
        outcome: "pass".into(),
        artifact_digest: Some(report.receipt_hash),
    });
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.into(),
        handoff: admit_candidate_handoff(work, attempt, handoff).map_err(text)?,
        work_identity: handoff.work_identity.clone(),
        work_revision: handoff.work_revision,
        attempt: handoff.attempt_identity.clone(),
        fence: handoff.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: handoff.verification_profile.clone(),
        run,
    };
    admit_candidate_verification(work, attempt, handoff, &verification).map_err(text)?;
    Ok(verification)
}

fn adapt_observation(
    row: &kiln_implementer::verification::CheckObservation,
    contract: &AgentVerificationContract,
) -> AgentExecutionResult {
    AgentExecutionResult {
        check_id: row.check_id.clone(),
        status: row.status.clone(),
        command: row.command.clone(),
        command_hash: cairn_core::authoritative_command_hash(&row.command),
        subject_hash: contract.final_subject_hash.clone(),
        registry_hash: contract.registry_hash.clone(),
        selection_hash: contract.selection_hash.clone(),
        policy_hash: contract.policy_hash.clone(),
        evidence_hash: row.evidence_hash.clone(),
        execution_receipt_hash: row.execution_receipt_hash.clone(),
        artifacts: row
            .artifacts
            .iter()
            .map(|artifact| AgentArtifactReference {
                path: artifact.path.clone(),
                hash: artifact.hash.clone(),
            })
            .collect(),
        diagnostic: None,
    }
}

fn frozen_json(material: &FrozenProgram) -> Result<serde_json::Value, String> {
    const MAX_MATERIAL_BYTES: u64 = 16 * 1024 * 1024;
    if !material.path.is_absolute()
        || !material.path.starts_with("/nix/store")
        || !fs::canonicalize(&material.path)
            .map_err(text)?
            .starts_with("/nix/store")
    {
        return Err("Cairn policy/registry material must be immutable admitted inputs".into());
    }
    let file = fs::File::open(&material.path).map_err(text)?;
    if !file.metadata().map_err(text)?.is_file() {
        return Err("Cairn frozen material is not regular".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_MATERIAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > MAX_MATERIAL_BYTES
        || blake3::hash(&bytes).to_hex().as_str() != material.blake3
    {
        return Err("Cairn current policy/registry bytes differ from admitted material".into());
    }
    serde_json::from_slice(&bytes).map_err(text)
}
fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_core::{
        AgentRequiredCheck, authoritative_command_hash, authoritative_selection_hash,
    };
    use serde_json::json;

    fn fixture() -> (serde_json::Value, serde_json::Value) {
        let command = vec!["/nix/store/admitted-check/bin/check".into()];
        let checks = vec![AgentRequiredCheck {
            check_id: "acceptance".into(),
            command_hash: authoritative_command_hash(&command),
            command: command.clone(),
            prerequisites: vec![],
        }];
        let contract = json!(AgentVerificationContract {
            schema: "cairn.agent-completion.v2".into(),
            repository_id: "repo".into(),
            registry_id: "frozen".into(),
            registry_hash: "a".repeat(64),
            policy_hash: "b".repeat(64),
            final_subject_hash: "c".repeat(64),
            selection_hash: authoritative_selection_hash(&checks),
            required_checks: checks
        });
        let results = json!([AgentExecutionResult {
            check_id: "acceptance".into(),
            status: "passed".into(),
            command_hash: authoritative_command_hash(&command),
            command,
            subject_hash: "c".repeat(64),
            registry_hash: "a".repeat(64),
            selection_hash: contract
                .get("selection_hash")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| unreachable!())
                .into(),
            policy_hash: "b".repeat(64),
            evidence_hash: "d".repeat(64),
            execution_receipt_hash: "e".repeat(64),
            artifacts: vec![AgentArtifactReference {
                path: "evidence/result.json".into(),
                hash: "f".repeat(64)
            }],
            diagnostic: None
        }]);
        (contract, results)
    }

    #[test]
    fn native_authoritative_evaluator_accepts_exact_executed_binding() {
        let (contract, results) = fixture();
        assert!(
            evaluate_authoritative_agent_completion(
                &contract,
                &results,
                &"c".repeat(64),
                &"b".repeat(64)
            )
            .valid
        );
    }

    #[test]
    fn native_evaluator_refuses_stale_duplicate_missing_failed_and_substituted_rows() {
        let (contract, results) = fixture();
        for field in [
            "subject_hash",
            "registry_hash",
            "selection_hash",
            "policy_hash",
            "command_hash",
        ] {
            let mut forged = results.clone();
            *forged
                .get_mut(0)
                .and_then(|row| row.get_mut(field))
                .unwrap_or_else(|| unreachable!()) = json!("0".repeat(64));
            assert!(
                !evaluate_authoritative_agent_completion(
                    &contract,
                    &forged,
                    &"c".repeat(64),
                    &"b".repeat(64)
                )
                .valid,
                "{field}"
            );
        }
        for status in [
            "failed",
            "skipped",
            "timed_out",
            "unknown",
            "infrastructure_error",
        ] {
            let mut failed = results.clone();
            *failed
                .get_mut(0)
                .and_then(|row| row.get_mut("status"))
                .unwrap_or_else(|| unreachable!()) = json!(status);
            assert!(
                !evaluate_authoritative_agent_completion(
                    &contract,
                    &failed,
                    &"c".repeat(64),
                    &"b".repeat(64)
                )
                .valid
            );
        }
        let first = results.get(0).unwrap_or_else(|| unreachable!());
        let duplicate = json!([first, first]);
        for rows in [json!([]), duplicate] {
            assert!(
                !evaluate_authoritative_agent_completion(
                    &contract,
                    &rows,
                    &"c".repeat(64),
                    &"b".repeat(64)
                )
                .valid
            );
        }
    }

    #[test]
    fn observation_adapter_preserves_receipts_artifacts_and_nonpassing_status() {
        use kiln_implementer::verification::{ArtifactObservation, CheckObservation};
        let (contract, _) = fixture();
        let contract: AgentVerificationContract =
            serde_json::from_value(contract).unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut observation = CheckObservation {
            check_id: "acceptance".into(),
            command: contract
                .required_checks
                .first()
                .unwrap_or_else(|| unreachable!())
                .command
                .clone(),
            status: "passed".into(),
            exit_code: Some(0),
            stdout_hash: "1".repeat(64),
            stderr_hash: "2".repeat(64),
            artifacts: vec![ArtifactObservation {
                path: "evidence/result.json".into(),
                hash: "f".repeat(64),
                bytes: 12,
            }],
            evidence_hash: "d".repeat(64),
            execution_receipt_hash: "e".repeat(64),
            baseline: false,
        };
        let adapted = adapt_observation(&observation, &contract);
        assert_eq!(
            adapted.execution_receipt_hash,
            observation.execution_receipt_hash
        );
        let artifact = adapted.artifacts.first().unwrap_or_else(|| unreachable!());
        let expected_artifact = observation
            .artifacts
            .first()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(artifact.hash, expected_artifact.hash);
        assert!(
            evaluate_authoritative_agent_completion(
                &json!(contract),
                &json!([adapted]),
                &"c".repeat(64),
                &"b".repeat(64)
            )
            .valid
        );
        observation.status = "unknown".into();
        observation.exit_code = None;
        let adapted = adapt_observation(&observation, &contract);
        assert_eq!(adapted.status, "unknown");
        assert!(
            !evaluate_authoritative_agent_completion(
                &json!(contract),
                &json!([adapted]),
                &"c".repeat(64),
                &"b".repeat(64)
            )
            .valid
        );
    }
}
