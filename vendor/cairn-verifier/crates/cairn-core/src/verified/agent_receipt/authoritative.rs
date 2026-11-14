use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::verified::evidence::canonical_json_hash;

const AUTHORITATIVE_SCHEMA: &str = "cairn.agent-completion.v2";
const PASSING_STATUS: &str = "passed";
const MAX_DIAGNOSTIC_SUMMARY_BYTES: usize = 512;
const MAX_REPRODUCTION_ARGS: usize = 32;
const REQUIRED_NON_CLAIMS: [&str; 5] = [
    "completion validation does not prove check-oracle soundness",
    "completion validation does not prove honest command execution beyond supplied evidence",
    "completion validation does not prove model correctness",
    "completion validation does not prove whole-product correctness",
    "completion validation does not prove release eligibility outside the selected contract",
];
const SUPPORTED_NON_PASSING_STATUSES: [&str; 11] = [
    "failed",
    "blocked",
    "skipped",
    "deferred",
    "timed_out",
    "cancelled",
    "dry_run",
    "cleanup_failed",
    "infrastructure_error",
    "not_selected",
    "unknown",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentVerificationContract {
    pub schema: String,
    pub repository_id: String,
    pub registry_id: String,
    pub registry_hash: String,
    pub selection_hash: String,
    pub policy_hash: String,
    pub final_subject_hash: String,
    pub required_checks: Vec<AgentRequiredCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRequiredCheck {
    pub check_id: String,
    pub command: Vec<String>,
    pub command_hash: String,
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub check_id: String,
    pub status: String,
    pub command: Vec<String>,
    pub command_hash: String,
    pub subject_hash: String,
    pub registry_hash: String,
    pub selection_hash: String,
    pub policy_hash: String,
    pub evidence_hash: String,
    pub execution_receipt_hash: String,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifactReference>,
    #[serde(default)]
    pub diagnostic: Option<AgentDiagnosticEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArtifactReference {
    pub path: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDiagnosticEvidence {
    pub remediation_label: String,
    pub summary: String,
    pub reproduction_argv: Vec<String>,
    pub working_root: String,
    #[serde(default)]
    pub source: Option<AgentSourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSourceSpan {
    pub path: String,
    pub line: u64,
    pub column: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentCompletionDiagnostic {
    pub diagnostic_id: String,
    pub code: String,
    pub check_id: Option<String>,
    pub result_status: Option<String>,
    pub subject_hash: Option<String>,
    pub registry_hash: Option<String>,
    pub selection_hash: Option<String>,
    pub evidence_hash: Option<String>,
    pub execution_receipt_hash: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub severity: String,
    pub remediation_label: String,
    pub summary: String,
    pub reproduction_argv: Vec<String>,
    pub working_root: String,
    pub source: Option<AgentSourceSpan>,
    pub artifacts: Vec<AgentArtifactReference>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentNonPassingCheck {
    pub check_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentAuthoritativeCompletionReport {
    pub schema: String,
    pub classification: String,
    pub repository_id: String,
    pub registry_id: String,
    pub registry_hash: String,
    pub selection_hash: String,
    pub policy_hash: String,
    pub final_subject_hash: String,
    pub required_checks: Vec<String>,
    pub passed_checks: Vec<String>,
    pub non_passing_checks: Vec<AgentNonPassingCheck>,
    pub diagnostics: Vec<AgentCompletionDiagnostic>,
    pub non_claims: Vec<String>,
    pub input_hash: String,
    pub valid: bool,
    pub verdict: String,
    pub receipt_hash: String,
}

struct DiagnosticContext<'a> {
    check_id: Option<&'a str>,
    status: Option<&'a str>,
    result: Option<&'a AgentExecutionResult>,
    required: Option<&'a AgentRequiredCheck>,
    expected: Option<&'a str>,
    actual: Option<&'a str>,
}

// r[impl cairn.supersede.agent_run_receipts.execution_boundary]
// r[impl cairn.supersede.agent_run_receipts.execution_boundary.pure_core]
// r[impl cairn.supersede.agent_run_receipts.verification_contract]
// r[impl cairn.supersede.agent_run_receipts.verification_contract.match]
// r[impl cairn.supersede.agent_run_receipts.pass_only]
// r[impl cairn.supersede.agent_run_receipts.freshness]
// r[impl cairn.supersede.agent_run_receipts.completion_authority]
// r[impl cairn.supersede.agent_run_receipts.actionable_diagnostics]
// r[impl cairn.supersede.agent_run_receipts.actionable_diagnostics.failed_check]
// r[impl cairn.supersede.agent_run_receipts.actionable_diagnostics.non_claims]
// r[impl cairn.supersede.agent_run_receipts.authoritative_verification]
// r[impl cairn.supersede.agent_run_receipts.schema_migration.goldens]
pub fn evaluate_authoritative_agent_completion(
    contract_value: &Value,
    results_value: &Value,
    current_subject_hash: &str,
    current_policy_hash: &str,
) -> AgentAuthoritativeCompletionReport {
    let input_hash = canonical_json_hash(&json!({
        "contract": contract_value,
        "results": results_value,
        "current_subject_hash": current_subject_hash,
        "current_policy_hash": current_policy_hash,
    }));
    let contract = serde_json::from_value::<AgentVerificationContract>(contract_value.clone());
    let results = parse_results(results_value);
    match (contract, results) {
        (Ok(contract), Ok(results)) => evaluate_normalized(
            &contract,
            &results,
            current_subject_hash,
            current_policy_hash,
            input_hash,
        ),
        (contract, results) => {
            let mut diagnostics = Vec::new();
            if let Err(error) = contract {
                diagnostics.push(diagnostic(
                    "verification_contract.malformed",
                    "repository verification contract is malformed",
                    DiagnosticContext {
                        check_id: None,
                        status: None,
                        result: None,
                        required: None,
                        expected: Some(AUTHORITATIVE_SCHEMA),
                        actual: Some(&error.to_string()),
                    },
                ));
            }
            if let Err(error) = results {
                diagnostics.push(diagnostic(
                    "verification_results.malformed",
                    "saved execution results are malformed",
                    DiagnosticContext {
                        check_id: None,
                        status: None,
                        result: None,
                        required: None,
                        expected: Some("array of execution result rows"),
                        actual: Some(&error),
                    },
                ));
            }
            finalize_report(
                None,
                Vec::new(),
                Vec::new(),
                diagnostics,
                input_hash,
                current_subject_hash,
                current_policy_hash,
            )
        }
    }
}

pub fn authoritative_command_hash(command: &[String]) -> String {
    canonical_json_hash(&json!(command))
}

pub fn authoritative_selection_hash(required_checks: &[AgentRequiredCheck]) -> String {
    let mut rows = required_checks
        .iter()
        .map(|check| {
            let mut prerequisites = check.prerequisites.clone();
            prerequisites.sort();
            prerequisites.dedup();
            json!({
                "check_id": check.check_id,
                "command_hash": check.command_hash,
                "prerequisites": prerequisites,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(canonical_json_hash);
    canonical_json_hash(&json!(rows))
}

fn parse_results(value: &Value) -> Result<Vec<AgentExecutionResult>, String> {
    let rows = value.get("results").unwrap_or(value).clone();
    serde_json::from_value(rows).map_err(|error| error.to_string())
}

fn evaluate_normalized(
    contract: &AgentVerificationContract,
    results: &[AgentExecutionResult],
    current_subject_hash: &str,
    current_policy_hash: &str,
    input_hash: String,
) -> AgentAuthoritativeCompletionReport {
    let mut diagnostics = Vec::new();
    validate_contract(
        contract,
        current_subject_hash,
        current_policy_hash,
        &mut diagnostics,
    );

    let required_by_id = contract
        .required_checks
        .iter()
        .map(|check| (check.check_id.as_str(), check))
        .collect::<BTreeMap<_, _>>();
    let mut results_by_id: BTreeMap<&str, Vec<&AgentExecutionResult>> = BTreeMap::new();
    for result in results {
        results_by_id
            .entry(result.check_id.as_str())
            .or_default()
            .push(result);
        if !required_by_id.contains_key(result.check_id.as_str()) {
            diagnostics.push(diagnostic(
                "verification_result.unknown_check",
                "execution result names a check absent from the repository-owned selection",
                context_for_result(result, None, None, None),
            ));
        }
    }

    let mut base_passing = BTreeSet::new();
    let mut statuses = BTreeMap::new();
    for check in &contract.required_checks {
        let rows = results_by_id
            .get(check.check_id.as_str())
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            statuses.insert(check.check_id.clone(), "missing".to_string());
            diagnostics.push(diagnostic(
                "verification_result.missing",
                "required check has no saved execution result",
                DiagnosticContext {
                    check_id: Some(&check.check_id),
                    status: Some("missing"),
                    result: None,
                    required: Some(check),
                    expected: None,
                    actual: None,
                },
            ));
            continue;
        }
        if rows.len() != 1 {
            statuses.insert(check.check_id.clone(), "duplicate".to_string());
            diagnostics.push(diagnostic(
                "verification_result.duplicate",
                "required check must have exactly one saved execution result",
                DiagnosticContext {
                    check_id: Some(&check.check_id),
                    status: Some("duplicate"),
                    result: rows.first().copied(),
                    required: Some(check),
                    expected: Some("1"),
                    actual: Some(&rows.len().to_string()),
                },
            ));
            continue;
        }
        let result = rows[0];
        statuses.insert(check.check_id.clone(), result.status.clone());
        let before = diagnostics.len();
        validate_result(contract, check, result, &mut diagnostics);
        if result.status != PASSING_STATUS {
            diagnostics.push(diagnostic(
                if is_supported_non_passing_status(&result.status) {
                    "verification_result.non_passing"
                } else {
                    "verification_result.unsupported_status"
                },
                "required check status is not completion eligible",
                context_for_result(
                    result,
                    Some(check),
                    Some(PASSING_STATUS),
                    Some(&result.status),
                ),
            ));
        }
        if diagnostics.len() == before && result.status == PASSING_STATUS {
            base_passing.insert(check.check_id.clone());
        }
    }

    let mut passed = Vec::new();
    for check in &contract.required_checks {
        if !base_passing.contains(&check.check_id) {
            continue;
        }
        let unsatisfied = check
            .prerequisites
            .iter()
            .filter(|prerequisite| !base_passing.contains(*prerequisite))
            .cloned()
            .collect::<Vec<_>>();
        if unsatisfied.is_empty() {
            passed.push(check.check_id.clone());
        } else {
            statuses.insert(check.check_id.clone(), "blocked".to_string());
            for prerequisite in unsatisfied {
                diagnostics.push(diagnostic(
                    "verification_result.prerequisite_not_passing",
                    "a passed result is ineligible because its prerequisite is not passing",
                    DiagnosticContext {
                        check_id: Some(&check.check_id),
                        status: Some("blocked"),
                        result: results_by_id
                            .get(check.check_id.as_str())
                            .and_then(|rows| rows.first())
                            .copied(),
                        required: Some(check),
                        expected: Some(&prerequisite),
                        actual: Some(
                            statuses
                                .get(&prerequisite)
                                .map(String::as_str)
                                .unwrap_or("missing"),
                        ),
                    },
                ));
            }
        }
    }

    let mut non_passing = statuses
        .into_iter()
        .filter(|(check_id, _)| !passed.contains(check_id))
        .map(|(check_id, status)| AgentNonPassingCheck { check_id, status })
        .collect::<Vec<_>>();
    passed.sort();
    non_passing.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    finalize_report(
        Some(contract),
        passed,
        non_passing,
        diagnostics,
        input_hash,
        current_subject_hash,
        current_policy_hash,
    )
}

fn validate_contract(
    contract: &AgentVerificationContract,
    current_subject_hash: &str,
    current_policy_hash: &str,
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
) {
    compare_text(
        diagnostics,
        "verification_contract.schema_mismatch",
        "verification contract schema is unsupported",
        AUTHORITATIVE_SCHEMA,
        &contract.schema,
    );
    compare_hash(
        diagnostics,
        "verification_contract.subject_stale",
        "verification contract does not bind the current final subject",
        current_subject_hash,
        &contract.final_subject_hash,
    );
    compare_hash(
        diagnostics,
        "verification_contract.policy_drift",
        "verification contract does not bind the current generated policy",
        current_policy_hash,
        &contract.policy_hash,
    );
    require_hash(
        diagnostics,
        "verification_contract.registry_hash_invalid",
        "verification contract registry hash must be BLAKE3 hex",
        &contract.registry_hash,
    );
    let expected_selection = authoritative_selection_hash(&contract.required_checks);
    compare_hash(
        diagnostics,
        "verification_contract.selection_drift",
        "verification contract selection hash does not match required check material",
        &expected_selection,
        &contract.selection_hash,
    );
    if contract.repository_id.is_empty()
        || contract.registry_id.is_empty()
        || contract.required_checks.is_empty()
    {
        diagnostics.push(diagnostic(
            "verification_contract.empty_identity",
            "verification contract identities and required check set must be non-empty",
            empty_context(),
        ));
    }

    let mut ids = BTreeSet::new();
    for check in &contract.required_checks {
        if check.check_id.is_empty() || !ids.insert(check.check_id.as_str()) {
            diagnostics.push(diagnostic(
                "verification_contract.duplicate_check",
                "repository contract check IDs must be non-empty and unique",
                DiagnosticContext {
                    check_id: Some(&check.check_id),
                    status: None,
                    result: None,
                    required: Some(check),
                    expected: None,
                    actual: None,
                },
            ));
        }
        let expected_command_hash = authoritative_command_hash(&check.command);
        if check.command.is_empty() || check.command_hash != expected_command_hash {
            diagnostics.push(diagnostic(
                "verification_contract.command_hash_mismatch",
                "required check command hash must bind the exact argv",
                DiagnosticContext {
                    check_id: Some(&check.check_id),
                    status: None,
                    result: None,
                    required: Some(check),
                    expected: Some(&expected_command_hash),
                    actual: Some(&check.command_hash),
                },
            ));
        }
    }
    let all_ids = contract
        .required_checks
        .iter()
        .map(|check| check.check_id.as_str())
        .collect::<BTreeSet<_>>();
    for check in &contract.required_checks {
        for prerequisite in &check.prerequisites {
            if prerequisite == &check.check_id || !all_ids.contains(prerequisite.as_str()) {
                diagnostics.push(diagnostic(
                    "verification_contract.prerequisite_invalid",
                    "required check prerequisite must name a different selected check",
                    DiagnosticContext {
                        check_id: Some(&check.check_id),
                        status: None,
                        result: None,
                        required: Some(check),
                        expected: Some("selected non-self check ID"),
                        actual: Some(prerequisite),
                    },
                ));
            }
        }
    }
    if prerequisite_graph_has_cycle(&contract.required_checks) {
        diagnostics.push(diagnostic(
            "verification_contract.prerequisite_cycle",
            "required check prerequisite graph must be acyclic",
            empty_context(),
        ));
    }
}

fn validate_result(
    contract: &AgentVerificationContract,
    check: &AgentRequiredCheck,
    result: &AgentExecutionResult,
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
) {
    compare_result_hash(
        diagnostics,
        "verification_result.subject_stale",
        "execution result does not bind the final subject",
        &contract.final_subject_hash,
        &result.subject_hash,
        result,
        check,
    );
    compare_result_hash(
        diagnostics,
        "verification_result.registry_drift",
        "execution result registry hash differs from the contract",
        &contract.registry_hash,
        &result.registry_hash,
        result,
        check,
    );
    compare_result_hash(
        diagnostics,
        "verification_result.selection_drift",
        "execution result selection hash differs from the contract",
        &contract.selection_hash,
        &result.selection_hash,
        result,
        check,
    );
    compare_result_hash(
        diagnostics,
        "verification_result.policy_drift",
        "execution result policy hash differs from the contract",
        &contract.policy_hash,
        &result.policy_hash,
        result,
        check,
    );
    let expected_command_hash = authoritative_command_hash(&result.command);
    if result.command != check.command
        || result.command_hash != check.command_hash
        || result.command_hash != expected_command_hash
    {
        diagnostics.push(diagnostic(
            "verification_result.command_mismatch",
            "execution result command identity differs from the required exact argv",
            context_for_result(
                result,
                Some(check),
                Some(&check.command_hash),
                Some(&result.command_hash),
            ),
        ));
    }
    require_result_hash(
        diagnostics,
        "verification_result.evidence_hash_invalid",
        "execution result evidence hash must be BLAKE3 hex",
        &result.evidence_hash,
        result,
        check,
    );
    require_result_hash(
        diagnostics,
        "verification_result.receipt_hash_invalid",
        "execution receipt hash must be BLAKE3 hex",
        &result.execution_receipt_hash,
        result,
        check,
    );
    for artifact in &result.artifacts {
        if !is_safe_relative_path(&artifact.path) || !is_blake3_hex(&artifact.hash) {
            diagnostics.push(diagnostic(
                "verification_result.artifact_invalid",
                "artifact references must use safe repository-relative paths and BLAKE3 hashes",
                context_for_result(result, Some(check), None, Some(&artifact.path)),
            ));
        }
    }
    if let Some(evidence) = &result.diagnostic {
        if !diagnostic_evidence_is_safe(evidence) {
            diagnostics.push(diagnostic(
                "verification_result.diagnostic_unsafe",
                "diagnostic evidence contains an unsafe or unbounded field",
                context_for_result(result, Some(check), None, None),
            ));
        }
    }
}

fn finalize_report(
    contract: Option<&AgentVerificationContract>,
    mut passed_checks: Vec<String>,
    mut non_passing_checks: Vec<AgentNonPassingCheck>,
    mut diagnostics: Vec<AgentCompletionDiagnostic>,
    input_hash: String,
    current_subject_hash: &str,
    current_policy_hash: &str,
) -> AgentAuthoritativeCompletionReport {
    diagnostics.sort_by(|left, right| {
        (&left.code, &left.check_id, &left.diagnostic_id).cmp(&(
            &right.code,
            &right.check_id,
            &right.diagnostic_id,
        ))
    });
    passed_checks.sort();
    non_passing_checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    let mut required_checks = contract
        .map(|value| {
            value
                .required_checks
                .iter()
                .map(|check| check.check_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    required_checks.sort();
    let valid = diagnostics.is_empty()
        && non_passing_checks.is_empty()
        && !required_checks.is_empty()
        && passed_checks == required_checks;
    let mut report = AgentAuthoritativeCompletionReport {
        schema: AUTHORITATIVE_SCHEMA.to_string(),
        classification: "authoritative".to_string(),
        repository_id: contract
            .map(|value| value.repository_id.clone())
            .unwrap_or_default(),
        registry_id: contract
            .map(|value| value.registry_id.clone())
            .unwrap_or_default(),
        registry_hash: contract
            .map(|value| value.registry_hash.clone())
            .unwrap_or_default(),
        selection_hash: contract
            .map(|value| value.selection_hash.clone())
            .unwrap_or_default(),
        policy_hash: current_policy_hash.to_string(),
        final_subject_hash: current_subject_hash.to_string(),
        required_checks,
        passed_checks,
        non_passing_checks,
        diagnostics,
        non_claims: non_claims(),
        input_hash,
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        receipt_hash: String::new(),
    };
    report.receipt_hash = canonical_report_hash(&report);
    report
}

fn diagnostic(
    code: &str,
    summary: &str,
    context: DiagnosticContext<'_>,
) -> AgentCompletionDiagnostic {
    let evidence = context.result.and_then(|result| result.diagnostic.as_ref());
    let remediation_label = evidence
        .map(|value| value.remediation_label.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "refresh-and-rerun-required-check".to_string());
    let bounded_summary = evidence
        .map(|value| value.summary.clone())
        .filter(|value| value.len() <= MAX_DIAGNOSTIC_SUMMARY_BYTES)
        .unwrap_or_else(|| summary.to_string());
    let reproduction_argv = evidence
        .map(|value| value.reproduction_argv.clone())
        .filter(|value| argv_is_safe(value))
        .or_else(|| context.required.map(|check| check.command.clone()))
        .unwrap_or_default();
    let working_root = evidence
        .map(|value| value.working_root.clone())
        .filter(|value| is_safe_root_label(value))
        .unwrap_or_else(|| "<repository-root>".to_string());
    let source = evidence
        .and_then(|value| value.source.clone())
        .filter(|value| is_safe_relative_path(&value.path));
    let artifacts = context
        .result
        .map(|result| {
            result
                .artifacts
                .iter()
                .filter(|artifact| {
                    is_safe_relative_path(&artifact.path) && is_blake3_hex(&artifact.hash)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let id_material = json!({
        "code": code,
        "check_id": context.check_id,
        "status": context.status,
        "expected": context.expected,
        "actual": context.actual,
    });
    AgentCompletionDiagnostic {
        diagnostic_id: canonical_json_hash(&id_material),
        code: code.to_string(),
        check_id: context.check_id.map(ToOwned::to_owned),
        result_status: context.status.map(ToOwned::to_owned),
        subject_hash: context.result.map(|value| value.subject_hash.clone()),
        registry_hash: context.result.map(|value| value.registry_hash.clone()),
        selection_hash: context.result.map(|value| value.selection_hash.clone()),
        evidence_hash: context.result.map(|value| value.evidence_hash.clone()),
        execution_receipt_hash: context
            .result
            .map(|value| value.execution_receipt_hash.clone()),
        expected: context.expected.map(ToOwned::to_owned),
        actual: context.actual.map(ToOwned::to_owned),
        severity: "error".to_string(),
        remediation_label,
        summary: bounded_summary,
        reproduction_argv,
        working_root,
        source,
        artifacts,
        non_claims: non_claims(),
    }
}

fn empty_context() -> DiagnosticContext<'static> {
    DiagnosticContext {
        check_id: None,
        status: None,
        result: None,
        required: None,
        expected: None,
        actual: None,
    }
}

fn context_for_result<'a>(
    result: &'a AgentExecutionResult,
    required: Option<&'a AgentRequiredCheck>,
    expected: Option<&'a str>,
    actual: Option<&'a str>,
) -> DiagnosticContext<'a> {
    DiagnosticContext {
        check_id: Some(&result.check_id),
        status: Some(&result.status),
        result: Some(result),
        required,
        expected,
        actual,
    }
}

fn compare_text(
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
    code: &str,
    summary: &str,
    expected: &str,
    actual: &str,
) {
    if expected != actual {
        diagnostics.push(diagnostic(
            code,
            summary,
            DiagnosticContext {
                expected: Some(expected),
                actual: Some(actual),
                ..empty_context()
            },
        ));
    }
}

fn compare_hash(
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
    code: &str,
    summary: &str,
    expected: &str,
    actual: &str,
) {
    if !is_blake3_hex(expected) || !is_blake3_hex(actual) || expected != actual {
        compare_text(diagnostics, code, summary, expected, actual);
    }
}

fn require_hash(
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
    code: &str,
    summary: &str,
    actual: &str,
) {
    if !is_blake3_hex(actual) {
        diagnostics.push(diagnostic(
            code,
            summary,
            DiagnosticContext {
                expected: Some("64-character BLAKE3 hex"),
                actual: Some(actual),
                ..empty_context()
            },
        ));
    }
}

fn compare_result_hash(
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
    code: &str,
    summary: &str,
    expected: &str,
    actual: &str,
    result: &AgentExecutionResult,
    check: &AgentRequiredCheck,
) {
    if !is_blake3_hex(expected) || !is_blake3_hex(actual) || expected != actual {
        diagnostics.push(diagnostic(
            code,
            summary,
            context_for_result(result, Some(check), Some(expected), Some(actual)),
        ));
    }
}

fn require_result_hash(
    diagnostics: &mut Vec<AgentCompletionDiagnostic>,
    code: &str,
    summary: &str,
    actual: &str,
    result: &AgentExecutionResult,
    check: &AgentRequiredCheck,
) {
    if !is_blake3_hex(actual) {
        diagnostics.push(diagnostic(
            code,
            summary,
            context_for_result(
                result,
                Some(check),
                Some("64-character BLAKE3 hex"),
                Some(actual),
            ),
        ));
    }
}

fn prerequisite_graph_has_cycle(checks: &[AgentRequiredCheck]) -> bool {
    let mut indegree = checks
        .iter()
        .map(|check| (check.check_id.as_str(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for check in checks {
        for prerequisite in &check.prerequisites {
            if let Some(value) = indegree.get_mut(check.check_id.as_str()) {
                *value += 1;
            }
            dependents
                .entry(prerequisite.as_str())
                .or_default()
                .push(check.check_id.as_str());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(next) = ready.pop_first() {
        visited += 1;
        for dependent in dependents.get(next).cloned().unwrap_or_default() {
            if let Some(value) = indegree.get_mut(dependent) {
                *value -= 1;
                if *value == 0 {
                    ready.insert(dependent);
                }
            }
        }
    }
    visited != checks.len()
}

fn diagnostic_evidence_is_safe(evidence: &AgentDiagnosticEvidence) -> bool {
    !evidence.remediation_label.is_empty()
        && evidence.summary.len() <= MAX_DIAGNOSTIC_SUMMARY_BYTES
        && argv_is_safe(&evidence.reproduction_argv)
        && is_safe_root_label(&evidence.working_root)
        && evidence
            .source
            .as_ref()
            .is_none_or(|source| is_safe_relative_path(&source.path))
}

fn argv_is_safe(argv: &[String]) -> bool {
    !argv.is_empty()
        && argv.len() <= MAX_REPRODUCTION_ARGS
        && argv.iter().all(|arg| {
            !arg.contains('\n')
                && !arg.contains('\r')
                && !arg.contains("SECRET")
                && !arg.contains("TOKEN=")
        })
}

fn is_safe_root_label(value: &str) -> bool {
    value == "<repository-root>" || value == "."
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.split('/').any(|part| part == "..")
        && !path.contains('\n')
        && !path.contains('\r')
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn is_supported_non_passing_status(status: &str) -> bool {
    SUPPORTED_NON_PASSING_STATUSES.contains(&status)
}

fn non_claims() -> Vec<String> {
    REQUIRED_NON_CLAIMS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn canonical_report_hash(report: &AgentAuthoritativeCompletionReport) -> String {
    let mut value = serde_json::to_value(report).expect("authoritative report serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("receipt_hash");
    }
    canonical_json_hash(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn fixture() -> (Value, Value) {
        let first_command = vec!["cargo".to_string(), "test".to_string()];
        let second_command = vec!["cargo".to_string(), "clippy".to_string()];
        let mut required_checks = vec![
            AgentRequiredCheck {
                check_id: "tests".to_string(),
                command: first_command.clone(),
                command_hash: authoritative_command_hash(&first_command),
                prerequisites: Vec::new(),
            },
            AgentRequiredCheck {
                check_id: "clippy".to_string(),
                command: second_command.clone(),
                command_hash: authoritative_command_hash(&second_command),
                prerequisites: vec!["tests".to_string()],
            },
        ];
        required_checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
        let selection_hash = authoritative_selection_hash(&required_checks);
        let contract = AgentVerificationContract {
            schema: AUTHORITATIVE_SCHEMA.to_string(),
            repository_id: "onixresearch/cairn".to_string(),
            registry_id: "cairn-release-checks".to_string(),
            registry_hash: HASH_A.to_string(),
            selection_hash: selection_hash.clone(),
            policy_hash: HASH_B.to_string(),
            final_subject_hash: HASH_C.to_string(),
            required_checks: required_checks.clone(),
        };
        let results = required_checks
            .iter()
            .map(|check| AgentExecutionResult {
                check_id: check.check_id.clone(),
                status: PASSING_STATUS.to_string(),
                command: check.command.clone(),
                command_hash: check.command_hash.clone(),
                subject_hash: HASH_C.to_string(),
                registry_hash: HASH_A.to_string(),
                selection_hash: selection_hash.clone(),
                policy_hash: HASH_B.to_string(),
                evidence_hash: HASH_A.to_string(),
                execution_receipt_hash: HASH_B.to_string(),
                artifacts: vec![AgentArtifactReference {
                    path: format!("target/{}.json", check.check_id),
                    hash: HASH_C.to_string(),
                }],
                diagnostic: Some(AgentDiagnosticEvidence {
                    remediation_label: "rerun-check".to_string(),
                    summary: "rerun the repository-owned check".to_string(),
                    reproduction_argv: check.command.clone(),
                    working_root: "<repository-root>".to_string(),
                    source: None,
                }),
            })
            .collect::<Vec<_>>();
        (
            serde_json::to_value(contract).unwrap(),
            serde_json::to_value(results).unwrap(),
        )
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.authoritative_verification.positive]
    // r[verify cairn.supersede.agent_run_receipts.pass_only.complete]
    // r[verify cairn.supersede.agent_run_receipts.freshness.match]
    fn exact_current_pass_only_coverage_completes() {
        let (contract, results) = fixture();
        let report = evaluate_authoritative_agent_completion(&contract, &results, HASH_C, HASH_B);
        assert!(report.valid, "{:#?}", report.diagnostics);
        assert_eq!(report.passed_checks, vec!["clippy", "tests"]);
        assert!(report.non_passing_checks.is_empty());
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.authoritative_verification.negative]
    // r[verify cairn.supersede.agent_run_receipts.pass_only.non_pass]
    fn every_policy_fixture_non_passing_status_fails_closed() {
        let matrix: Value = serde_json::from_str(include_str!(
            "../../../../../fixtures/agent-completion/negative-matrix.json"
        ))
        .unwrap();
        let statuses = matrix["non_passing_statuses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(statuses, SUPPORTED_NON_PASSING_STATUSES);
        for status in statuses {
            let (contract, mut results) = fixture();
            results[0]["status"] = Value::String(status.to_string());
            let report =
                evaluate_authoritative_agent_completion(&contract, &results, HASH_C, HASH_B);
            assert!(!report.valid, "status {status} unexpectedly passed");
            assert!(report
                .non_passing_checks
                .iter()
                .any(|row| row.status == status));
        }
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.pass_only.row_identity]
    // r[verify cairn.supersede.agent_run_receipts.pass_only.prerequisites]
    fn duplicate_unknown_and_failed_prerequisite_rows_fail_closed() {
        let (contract, mut results) = fixture();
        let duplicate = results[1].clone();
        results.as_array_mut().unwrap().push(duplicate);
        let mut unknown = results[1].clone();
        unknown["check_id"] = Value::String("unknown".to_string());
        results.as_array_mut().unwrap().push(unknown);
        let test_row = results
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["check_id"] == "tests")
            .unwrap();
        test_row["status"] = Value::String("failed".to_string());
        let report = evaluate_authoritative_agent_completion(&contract, &results, HASH_C, HASH_B);
        assert!(!report.valid);
        let codes = report
            .diagnostics
            .iter()
            .map(|row| row.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("verification_result.duplicate"));
        assert!(codes.contains("verification_result.unknown_check"));
        assert!(codes.contains("verification_result.prerequisite_not_passing"));
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.verification_contract.drift]
    // r[verify cairn.supersede.agent_run_receipts.freshness.post_check_mutation]
    // r[verify cairn.supersede.agent_run_receipts.freshness.hashes]
    fn stale_and_malformed_identity_evidence_fails_closed() {
        let (mut contract, mut results) = fixture();
        contract["selection_hash"] = Value::String(HASH_C.to_string());
        results[0]["subject_hash"] = Value::String(HASH_A.to_string());
        results[1]["evidence_hash"] = Value::String("bad".to_string());
        let report = evaluate_authoritative_agent_completion(&contract, &results, HASH_C, HASH_B);
        assert!(!report.valid);
        assert!(report
            .diagnostics
            .iter()
            .any(|row| row.code == "verification_contract.selection_drift"));
        assert!(report
            .diagnostics
            .iter()
            .any(|row| row.code == "verification_result.subject_stale"));
        assert!(report
            .diagnostics
            .iter()
            .any(|row| row.code == "verification_result.evidence_hash_invalid"));
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.actionable_diagnostics.safe]
    fn unsafe_diagnostic_is_rejected_without_raw_projection() {
        let (contract, mut results) = fixture();
        results[0]["diagnostic"]["working_root"] = Value::String("/home/private".to_string());
        results[0]["diagnostic"]["summary"] =
            Value::String("x".repeat(MAX_DIAGNOSTIC_SUMMARY_BYTES + 1));
        let report = evaluate_authoritative_agent_completion(&contract, &results, HASH_C, HASH_B);
        assert!(!report.valid);
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|row| row.code == "verification_result.diagnostic_unsafe")
            .unwrap();
        assert_eq!(diagnostic.working_root, "<repository-root>");
        assert!(diagnostic.summary.len() <= MAX_DIAGNOSTIC_SUMMARY_BYTES);
    }
}
