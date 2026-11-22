//! Pure convergence evaluation for `cairn convergence inspect`.
//!
//! The shell reads every input bounded, refuses links and parses JSON; this
//! module receives already-read facts and decides everything. It performs no
//! I/O, reads no environment, spawns no process and starts no evaluator.
//!
//! Acceptance is Octet's requirement decision only. Mantle success, cache
//! state, `proof_acceptance` and `externally_reported_outcome` are never
//! acceptance; the states stay advisory.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::assurance_targets::{
    bind_targets_to_registry, compare_projection, project_contract, AssuranceTarget,
    AssuranceTargetIssue, AssuranceTargetsPolicy, GeneratedSources, TargetContract, TargetsFile,
    ISSUE_STALE_ROW,
};
use super::content_bound_requirement_registry::ContentBoundRequirementRegistry;
use super::evidence::{blake3_hex, canonical_json};
use super::refinement_typed_dependencies::Assurance;

pub const CONVERGENCE_RECEIPT_SCHEMA_V1: &str = "cairn.convergence.v1";
pub const CONVERGENCE_CASE_SCHEMA_V1: &str = "cairn.convergence-case.v1";

/// The receipt states exactly what a convergence receipt does not grant.
pub const CONVERGENCE_AUTHORITY: &str = "advisory; Pass | Block gates, readiness states, verification roles and implementation-ready output are unchanged";

pub const CONVERGENCE_STATE_UNTARGETED: &str = "untargeted";
pub const CONVERGENCE_STATE_NOT_PROJECTED: &str = "not_projected";
pub const CONVERGENCE_STATE_CURRENT: &str = "current";
pub const CONVERGENCE_STATE_UNREALIZED: &str = "unrealized";
pub const CONVERGENCE_STATE_STALE: &str = "stale";
pub const CONVERGENCE_STATE_FAILED: &str = "failed";
pub const CONVERGENCE_STATE_UNSUPPORTED: &str = "unsupported";
pub const CONVERGENCE_STATE_INCOMPLETE: &str = "incomplete";

pub const CONVERGENCE_ISSUE_REGISTRY_UNVERIFIED: &str = "convergence.registry_unverified";
pub const CONVERGENCE_ISSUE_REVISION_KIND: &str = "convergence.revision_kind";
pub const CONVERGENCE_ISSUE_STALE_ROW: &str = "convergence.stale_row";
pub const CONVERGENCE_ISSUE_SPECIFICATION_MISMATCH: &str = "convergence.specification_mismatch";
pub const CONVERGENCE_ISSUE_CONTRACT_INPUTS_MISSING: &str = "convergence.contract_inputs_missing";
pub const CONVERGENCE_ISSUE_PROJECTION_MISMATCH: &str = "convergence.projection_mismatch";
pub const CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_STALE: &str =
    "convergence.reviewed_specification_stale";
pub const CONVERGENCE_ISSUE_CONTRACT_BLAKE3_MISMATCH: &str = "convergence.contract_blake3_mismatch";
pub const CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_MISMATCH: &str =
    "convergence.reviewed_specification_mismatch";
pub const CONVERGENCE_ISSUE_EVIDENCE_BLAKE3_MISMATCH: &str = "convergence.evidence_blake3_mismatch";
pub const CONVERGENCE_ISSUE_EVIDENCE_CONTRACT_MISMATCH: &str =
    "convergence.evidence_contract_mismatch";
pub const CONVERGENCE_ISSUE_DECISION_CLAUSE_MISSING: &str = "convergence.decision_clause_missing";
pub const CONVERGENCE_ISSUE_AGGREGATE_FAILED: &str = "convergence.aggregate_failed";
pub const CONVERGENCE_ISSUE_UNKNOWN_CLAUSE_STATUS: &str = "convergence.unknown_clause_status";
pub const CONVERGENCE_ISSUE_DECISION_FAILED: &str = "convergence.decision_failed";
pub const CONVERGENCE_ISSUE_DECISION_STALE: &str = "convergence.decision_stale";
pub const CONVERGENCE_ISSUE_DECISION_UNREALIZED: &str = "convergence.decision_unrealized";
pub const CONVERGENCE_ISSUE_DECISION_UNSUPPORTED: &str = "convergence.decision_unsupported";
pub const CONVERGENCE_ISSUE_DECISION_INCOMPLETE: &str = "convergence.decision_incomplete";
pub const CONVERGENCE_ISSUE_PROOF_ACCEPTANCE_RELAY: &str = "convergence.proof_acceptance_relay";
pub const CONVERGENCE_ISSUE_REVISION_MISMATCH: &str = "convergence.revision_mismatch";
pub const CONVERGENCE_ISSUE_JOB_ID_MISMATCH: &str = "convergence.job_id_mismatch";
pub const CONVERGENCE_ISSUE_PRODUCER_JOB_IDENTITY_MISMATCH: &str =
    "convergence.producer_job_identity_mismatch";
pub const CONVERGENCE_ISSUE_TOOL_GRAPH_IDENTITY_MISMATCH: &str =
    "convergence.tool_graph_identity_mismatch";
pub const CONVERGENCE_ISSUE_RECEIPT_IDENTITY_MISMATCH: &str =
    "convergence.receipt_identity_mismatch";
pub const CONVERGENCE_ISSUE_PRODUCER_RECEIPT_MISMATCH: &str =
    "convergence.producer_receipt_mismatch";
pub const CONVERGENCE_ISSUE_PRODUCER_PROFILE_MISMATCH: &str =
    "convergence.producer_profile_mismatch";
pub const CONVERGENCE_ISSUE_PROOF_DECK_MISSING: &str = "convergence.proof_deck_missing";
pub const CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH: &str =
    "convergence.proof_card_receipt_mismatch";
pub const CONVERGENCE_ISSUE_REALIZATION_MISSING: &str = "convergence.realization_missing";
pub const CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE: &str = "convergence.realization_incomplete";
pub const CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH: &str =
    "convergence.realization_derivation_mismatch";
pub const CONVERGENCE_ISSUE_REALIZATION_OUTPUT_MISMATCH: &str =
    "convergence.realization_output_mismatch";
pub const CONVERGENCE_ISSUE_STORE_INFO_MISSING: &str = "convergence.store_info_missing";
pub const CONVERGENCE_ISSUE_STORE_INFO_PATH_MISSING: &str = "convergence.store_info_path_missing";
pub const CONVERGENCE_ISSUE_NAR_MISMATCH: &str = "convergence.nar_mismatch";
pub const CONVERGENCE_ISSUE_UNTRUSTED_SIGNER: &str = "convergence.untrusted_signer";

/// An already-read JSON file: its BLAKE3 identity and parsed value.
#[derive(Debug, Clone)]
pub struct FileFacts {
    pub blake3: String,
    pub value: Value,
}

/// An already-read raw file: its BLAKE3 identity and text when it is UTF-8.
#[derive(Debug, Clone)]
pub struct BytesFacts {
    pub blake3: String,
    pub text: Option<String>,
}

/// Bridge handoff directory facts for one contract.
#[derive(Debug, Clone)]
pub struct HandoffFacts {
    pub observation: FileFacts,
    pub job: FileFacts,
    pub receipt: BytesFacts,
    pub profile: BytesFacts,
    pub tool_graph: BytesFacts,
    pub root_derivation: BytesFacts,
    pub source_revision: BytesFacts,
}

/// Producer output directory facts the bridge consumed.
#[derive(Debug, Clone)]
pub struct ProducerOutputFacts {
    pub dir_name: String,
    pub receipt: BytesFacts,
    pub profile: BytesFacts,
    /// Deck facts keyed by the deck path named in the evidence bundle.
    pub decks: BTreeMap<String, FileFacts>,
}

/// Every fact one contract case entry contributes.
#[derive(Debug, Clone)]
pub struct ContractCaseFacts {
    pub contract_id: String,
    pub contract: FileFacts,
    pub decision: FileFacts,
    pub evidence: FileFacts,
    pub handoff: HandoffFacts,
    pub producer_output: ProducerOutputFacts,
    pub realization: Option<FileFacts>,
    pub store_info: Option<FileFacts>,
}

/// The complete, already-read input to one convergence evaluation.
#[derive(Debug, Clone)]
pub struct ConvergenceRequest {
    pub policy: AssuranceTargetsPolicy,
    pub policy_blake3: String,
    pub case_blake3: String,
    pub registry: ContentBoundRequirementRegistry,
    pub registry_blake3: String,
    pub registry_verification_issues: Vec<String>,
    pub targets: TargetsFile,
    pub targets_blake3: String,
    pub generated: GeneratedSources,
    pub generated_blake3: String,
    /// Plain BLAKE3 of each accepted specification under `--root`, keyed by its
    /// repo-relative path. Octet verifies these bytes, not the registry's
    /// domain-separated identity.
    pub root_specification_digests: BTreeMap<String, String>,
    pub trusted_signers: Vec<String>,
    pub contracts: Vec<ContractCaseFacts>,
}

/// Closed state rank, weakest (`current`) to strongest (`incomplete`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rank {
    Current,
    Failed,
    Stale,
    Unrealized,
    Unsupported,
    Incomplete,
}

impl Rank {
    fn level(self) -> u8 {
        match self {
            Rank::Current => 0,
            Rank::Failed => 1,
            Rank::Stale => 2,
            Rank::Unrealized => 3,
            Rank::Unsupported => 4,
            Rank::Incomplete => 5,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Rank::Current => CONVERGENCE_STATE_CURRENT,
            Rank::Failed => CONVERGENCE_STATE_FAILED,
            Rank::Stale => CONVERGENCE_STATE_STALE,
            Rank::Unrealized => CONVERGENCE_STATE_UNREALIZED,
            Rank::Unsupported => CONVERGENCE_STATE_UNSUPPORTED,
            Rank::Incomplete => CONVERGENCE_STATE_INCOMPLETE,
        }
    }
}

/// Issues and the strongest state seen so far for one target.
#[derive(Debug)]
struct Evaluation {
    issues: Vec<String>,
    state: Rank,
}

impl Evaluation {
    fn new() -> Self {
        Evaluation {
            issues: Vec::new(),
            state: Rank::Current,
        }
    }

    fn mark(&mut self, rank: Rank, code: &str) {
        self.issues.push(code.to_string());
        if rank.level() > self.state.level() {
            self.state = rank;
        }
    }

    fn finish(mut self) -> Self {
        self.issues.sort();
        self.issues.dedup();
        self
    }
}

/// One registry row's reported outcome.
struct RowOutcome {
    state: &'static str,
    issues: Vec<String>,
    bound: Option<Value>,
    assurance: Option<String>,
    contract_id: Option<String>,
    job_id: Option<String>,
}

struct RealizationBinding {
    target_path: Option<String>,
    nar_sha256: Option<String>,
}

/// Evaluate every registry row in registry order and seal the receipt.
///
/// `targets_current` requires at least one target and every targeted row
/// `current` or `not_projected`; `coverage_complete` requires no untargeted
/// row; `complete` requires both.
///
pub fn evaluate_convergence(request: &ConvergenceRequest) -> Value {
    let bind_issues = bind_targets_to_registry(
        &request.targets,
        &request.registry.rows,
        &request.registry.specifications,
    );
    let mut rows = Vec::with_capacity(request.registry.rows.len());
    let mut targeted = 0usize;
    let mut all_targets_current = true;
    let mut any_untargeted = false;
    // A binding issue is consumed only by the row or specification the
    // evaluation actually looked at. Issues for targets whose requirement_id is
    // absent from the registry (or for a contract with no located target) are
    // never consumed, so they must still make the repository incomplete.
    let mut consumed_bind_paths = BTreeSet::new();

    for row in &request.registry.rows {
        let located = locate_target(request, &row.requirement_id);
        let outcome = match located {
            None => {
                any_untargeted = true;
                RowOutcome {
                    state: CONVERGENCE_STATE_UNTARGETED,
                    issues: Vec::new(),
                    bound: None,
                    assurance: None,
                    contract_id: None,
                    job_id: None,
                }
            }
            Some(located) => {
                targeted += 1;
                consumed_bind_paths.insert(format!(
                    "contracts[{}].targets[{}].requirement",
                    located.contract_index, located.target_index
                ));
                consumed_bind_paths.insert(format!(
                    "contracts[{}].specification",
                    located.contract_index
                ));
                let outcome = evaluate_target(request, &located, &bind_issues);
                if outcome.state != CONVERGENCE_STATE_CURRENT
                    && outcome.state != CONVERGENCE_STATE_NOT_PROJECTED
                {
                    all_targets_current = false;
                }
                outcome
            }
        };
        rows.push(json!({
            "requirement_id": row.requirement_id,
            "specification_path": row.specification_path,
            "row_blake3": row.row_blake3,
            "state": outcome.state,
            "assurance": outcome.assurance,
            "contract_id": outcome.contract_id,
            "job_id": outcome.job_id,
            "bound": outcome.bound,
            "issues": outcome.issues,
        }));
    }

    let mut unbound: Vec<(String, String)> = bind_issues
        .iter()
        .filter(|issue| !consumed_bind_paths.contains(&issue.field_path))
        .map(|issue| (issue.code.clone(), issue.field_path.clone()))
        .collect();
    unbound.sort();
    unbound.dedup();
    let all_bind_issues_consumed = unbound.is_empty();
    let unbound_target_issues: Vec<Value> = unbound
        .into_iter()
        .map(|(code, field_path)| json!({"code": code, "field_path": field_path}))
        .collect();

    let (targets_current, coverage_complete, complete) = completeness_flags(
        targeted,
        all_targets_current && all_bind_issues_consumed,
        any_untargeted,
    );

    let contract_inputs = request
        .contracts
        .iter()
        .map(|case| {
            json!({
                "contract_id": case.contract_id,
                "contract_blake3": case.contract.blake3,
                "decision_blake3": case.decision.blake3,
                "evidence_blake3": case.evidence.blake3,
                "observation_blake3": case.handoff.observation.blake3,
                "job_blake3": case.handoff.job.blake3,
                "receipt_blake3": case.handoff.receipt.blake3,
                "realization_blake3": case.realization.as_ref().map(|facts| facts.blake3.clone()),
                "store_info_blake3": case.store_info.as_ref().map(|facts| facts.blake3.clone()),
            })
        })
        .collect::<Vec<_>>();

    let mut receipt = json!({
        "schema_version": CONVERGENCE_RECEIPT_SCHEMA_V1,
        "status": "evaluated",
        "repository_id": request.registry.repository_id,
        "revision": {
            "kind": request.registry.revision.kind,
            "value": request.registry.revision.value,
        },
        "inputs": {
            "case_blake3": request.case_blake3,
            "policy_blake3": request.policy_blake3,
            "registry_blake3": request.registry_blake3,
            "targets_blake3": request.targets_blake3,
            "generated_sources_blake3": request.generated_blake3,
            "contracts": contract_inputs,
        },
        "rows": rows,
        "unbound_target_issues": unbound_target_issues,
        "targets_current": targets_current,
        "coverage_complete": coverage_complete,
        "complete": complete,
        "authority": CONVERGENCE_AUTHORITY,
        "non_claims": request.policy.non_claims,
    });
    seal_receipt(&mut receipt);
    receipt
}

struct LocatedTarget<'a> {
    contract: &'a TargetContract,
    target: &'a AssuranceTarget,
    contract_index: usize,
    target_index: usize,
}

fn locate_target<'a>(
    request: &'a ConvergenceRequest,
    requirement_id: &str,
) -> Option<LocatedTarget<'a>> {
    for (contract_index, contract) in request.targets.contracts.iter().enumerate() {
        if let Some(target_index) = contract
            .targets
            .iter()
            .position(|target| target.requirement.requirement_id == requirement_id)
        {
            let target = contract.targets.get(target_index)?;
            return Some(LocatedTarget {
                contract,
                target,
                contract_index,
                target_index,
            });
        }
    }
    None
}

/// r[impl cairn.convergence.completeness]
fn completeness_flags(
    targeted: usize,
    all_targets_current: bool,
    any_untargeted: bool,
) -> (bool, bool, bool) {
    let targets_current = targeted > 0 && all_targets_current;
    let coverage_complete = !any_untargeted;
    (
        targets_current,
        coverage_complete,
        targets_current && coverage_complete,
    )
}

fn evaluate_target(
    request: &ConvergenceRequest,
    located: &LocatedTarget<'_>,
    bind_issues: &[AssuranceTargetIssue],
) -> RowOutcome {
    let target = located.target;
    let requirement_id = target.requirement.requirement_id.clone();
    let mut evaluation = Evaluation::new();
    check_binding(&mut evaluation, located, bind_issues);

    if target.assurance == Assurance::Reviewed {
        let state = if evaluation.state == Rank::Stale {
            CONVERGENCE_STATE_STALE
        } else {
            CONVERGENCE_STATE_NOT_PROJECTED
        };
        return RowOutcome {
            state,
            issues: evaluation.finish().issues,
            bound: None,
            assurance: Some(assurance_name(target.assurance)),
            contract_id: Some(located.contract.contract_id.clone()),
            job_id: target.evidence_job.as_ref().map(|job| job.job_id.clone()),
        };
    }

    check_registry(request, &mut evaluation);
    let Some(case) = request
        .contracts
        .iter()
        .find(|case| case.contract_id == located.contract.contract_id)
    else {
        evaluation.mark(Rank::Incomplete, CONVERGENCE_ISSUE_CONTRACT_INPUTS_MISSING);
        return RowOutcome {
            state: evaluation.state.name(),
            issues: evaluation.finish().issues,
            bound: None,
            assurance: Some(assurance_name(target.assurance)),
            contract_id: Some(located.contract.contract_id.clone()),
            job_id: target.evidence_job.as_ref().map(|job| job.job_id.clone()),
        };
    };

    check_projection(request, located.contract, case, &mut evaluation);
    check_reviewed_specification(request, case, &mut evaluation);
    check_decision_binding(case, &mut evaluation);
    check_decision_acceptance(request, case, &requirement_id, &mut evaluation);
    check_handoff(request, target, case, &mut evaluation);
    check_proof_card(case, target, &mut evaluation);
    let binding = check_realization(request, case, &mut evaluation);
    let signer = check_store_info(request, case, &binding, &mut evaluation);

    let mut outcome = RowOutcome {
        state: evaluation.state.name(),
        issues: evaluation.finish().issues,
        bound: None,
        assurance: Some(assurance_name(target.assurance)),
        contract_id: Some(located.contract.contract_id.clone()),
        job_id: target.evidence_job.as_ref().map(|job| job.job_id.clone()),
    };
    if outcome.state == CONVERGENCE_STATE_CURRENT {
        outcome.bound = Some(json!({
            "source_revision": request.registry.revision.value,
            "producer_job_identity": observed_string(&case.handoff.observation.value, "producer_job_identity").unwrap_or_default(),
            "receipt_identity": observed_string(&case.handoff.observation.value, "receipt_identity").unwrap_or_default(),
            "root_derivation": case.handoff.root_derivation.text.as_deref().map(str::trim).unwrap_or_default(),
            "target_path": binding.target_path.unwrap_or_default(),
            "signer": signer.unwrap_or_default(),
        }));
    }
    outcome
}

fn check_binding(
    evaluation: &mut Evaluation,
    located: &LocatedTarget<'_>,
    bind_issues: &[AssuranceTargetIssue],
) {
    let row_path = format!(
        "contracts[{}].targets[{}].requirement",
        located.contract_index, located.target_index
    );
    let specification_path = format!("contracts[{}].specification", located.contract_index);
    for issue in bind_issues {
        if issue.field_path == row_path || issue.field_path == specification_path {
            let code = if issue.code == ISSUE_STALE_ROW {
                CONVERGENCE_ISSUE_STALE_ROW
            } else {
                CONVERGENCE_ISSUE_SPECIFICATION_MISMATCH
            };
            evaluation.mark(Rank::Stale, code);
        }
    }
}

fn check_registry(request: &ConvergenceRequest, evaluation: &mut Evaluation) {
    if !request.registry_verification_issues.is_empty() {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REGISTRY_UNVERIFIED);
    }
    if request.registry.revision.kind != "git" {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REVISION_KIND);
    }
}

fn check_projection(
    request: &ConvergenceRequest,
    contract: &TargetContract,
    case: &ContractCaseFacts,
    evaluation: &mut Evaluation,
) {
    match project_contract(contract, &request.generated, &request.policy) {
        Ok(projected) => {
            if !compare_projection(&projected, &case.contract.value).is_empty() {
                evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_PROJECTION_MISMATCH);
            }
        }
        Err(_) => evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_PROJECTION_MISMATCH),
    }
}

/// Octet judges the contract's reviewed specification against the plain BLAKE3
/// of the accepted file under `--root`; the registry's domain-separated
/// identity does not satisfy it.
fn check_reviewed_specification(
    request: &ConvergenceRequest,
    case: &ContractCaseFacts,
    evaluation: &mut Evaluation,
) {
    let reviewed = case
        .contract
        .value
        .get("reviewed_specification")
        .and_then(Value::as_object);
    let path = reviewed
        .and_then(|specification| specification.get("path"))
        .and_then(Value::as_str);
    let blake3 = reviewed
        .and_then(|specification| specification.get("blake3"))
        .and_then(Value::as_str);
    let matches = match (path, blake3) {
        (Some(path), Some(blake3)) => request
            .root_specification_digests
            .get(path)
            .is_some_and(|plain| plain == blake3),
        _ => false,
    };
    if !matches {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_STALE);
    }
}

fn check_decision_binding(case: &ContractCaseFacts, evaluation: &mut Evaluation) {
    let contract_digest = case.contract.blake3.as_str();
    if !observed_digest_matches(case.decision.value.get("contract_blake3"), contract_digest) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_CONTRACT_BLAKE3_MISMATCH);
    }
    let reviewed_specification = case
        .decision
        .value
        .get("reviewed_specification")
        .and_then(Value::as_object);
    let expected_specification = case
        .contract
        .value
        .get("reviewed_specification")
        .and_then(Value::as_object);
    if reviewed_specification != expected_specification || expected_specification.is_none() {
        evaluation.mark(
            Rank::Stale,
            CONVERGENCE_ISSUE_REVIEWED_SPECIFICATION_MISMATCH,
        );
    }
    if !observed_digest_matches(
        case.decision.value.get("evidence_blake3"),
        &case.evidence.blake3,
    ) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_EVIDENCE_BLAKE3_MISMATCH);
    }
    if !observed_digest_matches(case.evidence.value.get("contract_blake3"), contract_digest) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_EVIDENCE_CONTRACT_MISMATCH);
    }
}

/// Only the decision clause (with a passing aggregate) can accept a target.
///
/// r[impl cairn.convergence.acceptance]
fn check_decision_acceptance(
    request: &ConvergenceRequest,
    case: &ContractCaseFacts,
    requirement_id: &str,
    evaluation: &mut Evaluation,
) {
    let clause = case
        .decision
        .value
        .get("clauses")
        .and_then(Value::as_array)
        .and_then(|clauses| {
            clauses.iter().find(|clause| {
                clause.get("clause_id").and_then(Value::as_str) == Some(requirement_id)
            })
        });
    let Some(clause) = clause else {
        evaluation.mark(Rank::Incomplete, CONVERGENCE_ISSUE_DECISION_CLAUSE_MISSING);
        return;
    };
    let status = clause.get("status").and_then(Value::as_str);
    let aggregate = case
        .decision
        .value
        .get("aggregate_result")
        .and_then(Value::as_str);
    if aggregate != Some(request.policy.octet.passing_aggregate.as_str())
        && status == Some("passed")
    {
        evaluation.mark(Rank::Failed, CONVERGENCE_ISSUE_AGGREGATE_FAILED);
        return;
    }
    let Some(status) = status else {
        evaluation.mark(Rank::Unsupported, CONVERGENCE_ISSUE_UNKNOWN_CLAUSE_STATUS);
        return;
    };
    match request.policy.octet.clause_states.get(status) {
        None => evaluation.mark(Rank::Unsupported, CONVERGENCE_ISSUE_UNKNOWN_CLAUSE_STATUS),
        Some(mapped) => match decision_rank(mapped) {
            Some(Rank::Current) => {}
            Some(rank) => evaluation.mark(rank, decision_issue_code(mapped)),
            None => evaluation.mark(Rank::Unsupported, CONVERGENCE_ISSUE_UNKNOWN_CLAUSE_STATUS),
        },
    }
}

/// r[impl cairn.convergence.states]
fn decision_rank(mapped: &str) -> Option<Rank> {
    match mapped {
        CONVERGENCE_STATE_CURRENT => Some(Rank::Current),
        CONVERGENCE_STATE_FAILED => Some(Rank::Failed),
        CONVERGENCE_STATE_STALE => Some(Rank::Stale),
        CONVERGENCE_STATE_UNREALIZED => Some(Rank::Unrealized),
        CONVERGENCE_STATE_UNSUPPORTED => Some(Rank::Unsupported),
        CONVERGENCE_STATE_INCOMPLETE => Some(Rank::Incomplete),
        _ => None,
    }
}

fn decision_issue_code(mapped: &str) -> &'static str {
    match mapped {
        CONVERGENCE_STATE_FAILED => CONVERGENCE_ISSUE_DECISION_FAILED,
        CONVERGENCE_STATE_UNREALIZED => CONVERGENCE_ISSUE_DECISION_UNREALIZED,
        CONVERGENCE_STATE_UNSUPPORTED => CONVERGENCE_ISSUE_DECISION_UNSUPPORTED,
        CONVERGENCE_STATE_INCOMPLETE => CONVERGENCE_ISSUE_DECISION_INCOMPLETE,
        _ => CONVERGENCE_ISSUE_DECISION_STALE,
    }
}

fn check_handoff(
    request: &ConvergenceRequest,
    target: &AssuranceTarget,
    case: &ContractCaseFacts,
    evaluation: &mut Evaluation,
) {
    let observation = &case.handoff.observation.value;
    let proof_acceptance = observation.get("proof_acceptance").and_then(Value::as_str);
    if proof_acceptance != Some(request.policy.mantle.proof_acceptance_relay.as_str()) {
        evaluation.mark(Rank::Unsupported, CONVERGENCE_ISSUE_PROOF_ACCEPTANCE_RELAY);
    }

    let registry_revision = request.registry.revision.value.as_str();
    let packaging_revision = observation
        .pointer("/packaging/source_revision")
        .and_then(Value::as_str);
    let handoff_revision = case.handoff.source_revision.text.as_deref().map(str::trim);
    if packaging_revision != Some(registry_revision) || handoff_revision != Some(registry_revision)
    {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REVISION_MISMATCH);
    }

    let job_id = case.handoff.job.value.get("job_id").and_then(Value::as_str);
    let target_job_id = target.evidence_job.as_ref().map(|job| job.job_id.as_str());
    if job_id.is_none() || job_id != target_job_id {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_JOB_ID_MISMATCH);
    }

    if !observed_digest_matches(
        observation.get("producer_job_identity"),
        &case.handoff.job.blake3,
    ) {
        evaluation.mark(
            Rank::Stale,
            CONVERGENCE_ISSUE_PRODUCER_JOB_IDENTITY_MISMATCH,
        );
    }
    if !observed_digest_matches(
        observation.get("tool_graph_identity"),
        &case.handoff.tool_graph.blake3,
    ) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_TOOL_GRAPH_IDENTITY_MISMATCH);
    }
    if !observed_digest_matches(
        observation.get("receipt_identity"),
        &case.handoff.receipt.blake3,
    ) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_RECEIPT_IDENTITY_MISMATCH);
    }
    if !digest_eq(
        &case.producer_output.receipt.blake3,
        &case.handoff.receipt.blake3,
    ) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_PRODUCER_RECEIPT_MISMATCH);
    }
    if !digest_eq(
        &case.producer_output.profile.blake3,
        &case.handoff.profile.blake3,
    ) {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_PRODUCER_PROFILE_MISMATCH);
    }
}

fn check_proof_card(
    case: &ContractCaseFacts,
    target: &AssuranceTarget,
    evaluation: &mut Evaluation,
) {
    let evidence_ids = target.evidence_ids.as_deref().unwrap_or(&[]);
    let observation_receipt = observed_string(&case.handoff.observation.value, "receipt_identity")
        .map(strip_digest_prefix)
        .unwrap_or_default();
    let mut accepted_card = false;

    if let Some(decks) = case
        .evidence
        .value
        .get("proof_decks")
        .and_then(Value::as_array)
    {
        for deck in decks {
            let evidence_id = deck.get("evidence_id").and_then(Value::as_str);
            let Some(evidence_id) = evidence_id else {
                continue;
            };
            if !evidence_ids.iter().any(|allowed| allowed == evidence_id) {
                continue;
            }
            let deck_path = deck.get("deck").and_then(Value::as_str);
            let Some(facts) = deck_path.and_then(|path| case.producer_output.decks.get(path))
            else {
                evaluation.mark(Rank::Incomplete, CONVERGENCE_ISSUE_PROOF_DECK_MISSING);
                continue;
            };
            let Some(cards) = facts.value.get("cards").and_then(Value::as_array) else {
                continue;
            };
            for card in cards {
                let property_allowed = card
                    .get("proof_or_property_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| evidence_ids.iter().any(|allowed| allowed == id));
                if !property_allowed {
                    continue;
                }
                let receipt_path = card.get("receipt_path").and_then(Value::as_str);
                let Some(receipt_path) = receipt_path else {
                    continue;
                };
                let Some(identities) = card
                    .get("artifact_blake3_identities")
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                for identity in identities {
                    let artifact = identity.get("artifact").and_then(Value::as_str);
                    let blake3 = identity.get("blake3").and_then(Value::as_str);
                    if artifact == Some(receipt_path)
                        && blake3.is_some_and(|value| digest_eq(value, observation_receipt))
                    {
                        accepted_card = true;
                    }
                }
            }
        }
    }
    if !accepted_card {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_PROOF_CARD_RECEIPT_MISMATCH);
    }
}

/// Bind Mantle's admitted root output to the directory the bridge consumed.
///
/// r[impl cairn.convergence.realization]
fn check_realization(
    request: &ConvergenceRequest,
    case: &ContractCaseFacts,
    evaluation: &mut Evaluation,
) -> RealizationBinding {
    let mut binding = RealizationBinding {
        target_path: None,
        nar_sha256: None,
    };
    let Some(realization) = case.realization.as_ref() else {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_REALIZATION_MISSING);
        return binding;
    };
    let value = &realization.value;
    let status = value.get("status").and_then(Value::as_str);
    let strongest_state = value.get("strongest_state").and_then(Value::as_str);
    let state_realized = strongest_state.is_some_and(|state| {
        request
            .policy
            .mantle
            .realized_states
            .iter()
            .any(|realized| realized == state)
    });
    if status != Some(request.policy.mantle.realization_complete_status.as_str()) || !state_realized
    {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE);
    }

    let root_derivation = case
        .handoff
        .root_derivation
        .text
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    let unit = value
        .get("units")
        .and_then(Value::as_array)
        .and_then(|units| {
            units.iter().find(|unit| {
                unit.get("target_derivation").and_then(Value::as_str) == Some(root_derivation)
            })
        });
    let Some(unit) = unit else {
        evaluation.mark(
            Rank::Stale,
            CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH,
        );
        return binding;
    };
    let dir_name = case.producer_output.dir_name.as_str();
    let output = unit
        .get("outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| {
            outputs.iter().find(|output| {
                output
                    .get("target_path")
                    .and_then(Value::as_str)
                    .map(final_component)
                    == Some(dir_name)
            })
        });
    let Some(output) = output else {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REALIZATION_OUTPUT_MISMATCH);
        return binding;
    };
    binding.target_path = output
        .get("target_path")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.nar_sha256 = output
        .get("nar_sha256")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding
}

fn check_store_info(
    request: &ConvergenceRequest,
    case: &ContractCaseFacts,
    binding: &RealizationBinding,
    evaluation: &mut Evaluation,
) -> Option<String> {
    let Some(store_info) = case.store_info.as_ref() else {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_STORE_INFO_MISSING);
        return None;
    };
    let dir_name = case.producer_output.dir_name.as_str();
    let path_entry = store_info
        .value
        .get("paths")
        .and_then(Value::as_array)
        .and_then(|paths| {
            paths.iter().find(|path| {
                path.get("store_path")
                    .and_then(Value::as_str)
                    .map(final_component)
                    == Some(dir_name)
            })
        });
    let Some(path_entry) = path_entry else {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_STORE_INFO_PATH_MISSING);
        return None;
    };
    // A missing or non-string NAR hash on either side is a binding failure, not
    // an absent check: the store entry cannot be tied to the realized output.
    let nar_matches = binding.nar_sha256.as_deref().is_some_and(|expected| {
        path_entry
            .get("nar_sha256")
            .and_then(Value::as_str)
            .is_some_and(|nar| digest_eq(nar, expected))
    });
    if !nar_matches {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_NAR_MISMATCH);
    }
    let signer = path_entry
        .get("signatures")
        .and_then(Value::as_array)
        .and_then(|signatures| {
            signatures
                .iter()
                .filter_map(Value::as_str)
                .map(key_name)
                .find(|name| {
                    request
                        .trusted_signers
                        .iter()
                        .any(|trusted| trusted == name)
                })
        });
    match signer {
        Some(name) => Some(name.to_string()),
        None => {
            evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_UNTRUSTED_SIGNER);
            None
        }
    }
}

fn assurance_name(assurance: Assurance) -> String {
    match serde_json::to_value(assurance) {
        Ok(Value::String(name)) => name,
        _ => assurance_name_fallback(assurance),
    }
}

fn assurance_name_fallback(assurance: Assurance) -> String {
    match assurance {
        Assurance::Unassessed => "unassessed",
        Assurance::Reviewed => "reviewed",
        Assurance::BoundedCheck => "bounded_check",
        Assurance::CheckedProof => "checked_proof",
    }
    .to_string()
}

fn observed_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn observed_digest_matches(value: Option<&Value>, expected: &str) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|observed| digest_eq(observed, expected))
}

/// Digest comparisons ignore an optional `b3:` prefix on either side.
fn digest_eq(left: &str, right: &str) -> bool {
    strip_digest_prefix(left) == strip_digest_prefix(right)
}

fn strip_digest_prefix(value: &str) -> &str {
    value.strip_prefix("b3:").unwrap_or(value)
}

fn final_component(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A signer's key name is the text before the first `:`.
fn key_name(signature: &str) -> &str {
    signature.split(':').next().unwrap_or(signature)
}

/// Seal the receipt with the BLAKE3 of its canonical JSON, without itself.
///
/// r[impl cairn.convergence.determinism]
fn seal_receipt(receipt: &mut Value) {
    let receipt_blake3 = blake3_hex(canonical_json(receipt).as_bytes());
    if let Some(object) = receipt.as_object_mut() {
        object.insert("receipt_blake3".into(), Value::String(receipt_blake3));
    }
}

#[cfg(test)]
mod tests;
