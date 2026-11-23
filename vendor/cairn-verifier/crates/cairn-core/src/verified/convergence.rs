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
    admitted_producer_job_schema, bind_targets_to_registry, compare_projection, project_contract,
    AssuranceTarget, AssuranceTargetIssue, AssuranceTargetsPolicy, GeneratedSources,
    TargetContract, TargetsFile, ISSUE_STALE_ROW,
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
pub const CONVERGENCE_ISSUE_MISSING_PRODUCER_OWNER: &str = "convergence.missing_producer_owner";
pub const CONVERGENCE_ISSUE_UNKNOWN_PRODUCER_OWNER: &str = "convergence.unknown_producer_owner";
pub const CONVERGENCE_ISSUE_PRODUCER_SCHEMA_MISMATCH: &str = "convergence.producer_schema_mismatch";
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
pub const CONVERGENCE_ISSUE_REUSE_LINK_INVALID: &str = "convergence.reuse_link_invalid";

/// Mantle's realization receipt v2 schema. Version 1 stays the default for
/// every other route.
pub const REALIZATION_RECEIPT_SCHEMA_V2: &str = "mantle-foreign-realization-receipt-v2";
/// Version 1 records no root disposition; Cairn derives `preserved` from this
/// route because Mantle did not build or execute the root.
pub const CACHE_ONLY_PRESERVE_ROUTE: &str = "cache-only-preserve-v1";
/// Every version 1 build-route receipt means Mantle built the root.
pub const CACHE_INPUTS_BUILD_ROOTS_ROUTE_V1: &str = "cache-inputs-build-roots-v1";
pub const ROOT_DISPOSITION_BUILT: &str = "built";
pub const ROOT_DISPOSITION_REUSED: &str = "reused";
pub const ROOT_DISPOSITION_PRESERVED: &str = "preserved";
/// The only signer role that may attest a version 2 root.
pub const BUILDER_SIGNER_ROLE: &str = "builder";

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
    /// Absolute path the evidence recorded for the realized output when the
    /// retained copy lives elsewhere; absolute deck locators resolve against it.
    pub producer_output_locator: Option<String>,
    pub contract: FileFacts,
    pub decision: FileFacts,
    pub evidence: FileFacts,
    pub handoff: HandoffFacts,
    pub producer_output: ProducerOutputFacts,
    pub realization: Option<FileFacts>,
    /// The earlier version 2 receipt a `reused` root links to, when the case
    /// supplies it.
    pub original_realization: Option<FileFacts>,
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
    /// Builder public-key fingerprints (`public_key_blake3`) the case trusts.
    /// A version 2 root signer must name one of these.
    pub trusted_builder_keys: Vec<String>,
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
    node_id: Option<String>,
    target_derivation: Option<String>,
    target_path: Option<String>,
    nar_sha256: Option<String>,
    root_disposition: Option<String>,
    original_receipt_blake3: Option<String>,
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
                "producer_output_locator": case.producer_output_locator,
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
        // Mantle records the route it used on the realization receipt; a null or
        // non-string value states no route rather than inventing one.
        let realization_route = case
            .realization
            .as_ref()
            .and_then(|facts| facts.value.pointer("/cache_policy/realization_route"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        outcome.bound = Some(json!({
            "source_revision": request.registry.revision.value,
            "producer_job_identity": observed_string(&case.handoff.observation.value, "producer_job_identity").unwrap_or_default(),
            "receipt_identity": observed_string(&case.handoff.observation.value, "receipt_identity").unwrap_or_default(),
            "root_derivation": case.handoff.root_derivation.text.as_deref().map(str::trim).unwrap_or_default(),
            "node_id": binding.node_id.unwrap_or_default(),
            "target_derivation": binding.target_derivation.unwrap_or_default(),
            "target_path": binding.target_path.unwrap_or_default(),
            "signer": signer.unwrap_or_default(),
            "realization_route": realization_route,
            "root_disposition": binding.root_disposition,
            "original_receipt_blake3": binding.original_receipt_blake3,
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

/// A handoff producer job the policy does not admit for its own owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerJobAdmissionIssue {
    pub code: &'static str,
    pub message: String,
}

/// Admit the handoff `job.json` against the policy's owner-scoped map.
///
/// The job must name a non-empty `owner`; the policy must admit a schema for
/// that owner (matched without ASCII case); and the job's `schema_version`
/// must equal it. A missing owner, an unknown owner and a schema/owner
/// mismatch each fail with their own stable code.
pub fn admit_producer_job<'a>(
    policy: &'a AssuranceTargetsPolicy,
    job: &Value,
) -> Result<&'a str, ProducerJobAdmissionIssue> {
    let owner = job
        .get("owner")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|owner| !owner.is_empty());
    let Some(owner) = owner else {
        return Err(ProducerJobAdmissionIssue {
            code: CONVERGENCE_ISSUE_MISSING_PRODUCER_OWNER,
            message: "handoff job.json must declare a non-empty owner".to_string(),
        });
    };
    let Some(admitted) = admitted_producer_job_schema(policy, owner) else {
        return Err(ProducerJobAdmissionIssue {
            code: CONVERGENCE_ISSUE_UNKNOWN_PRODUCER_OWNER,
            message: format!(
                "policy admits no producer-job schema for owner {owner:?}; schemas.producer_jobs must name it"
            ),
        });
    };
    if job.get("schema_version").and_then(Value::as_str) != Some(admitted) {
        return Err(ProducerJobAdmissionIssue {
            code: CONVERGENCE_ISSUE_PRODUCER_SCHEMA_MISMATCH,
            message: format!(
                "handoff job.json schema_version must equal {admitted} for owner {owner}"
            ),
        });
    }
    Ok(admitted)
}

fn check_handoff(
    request: &ConvergenceRequest,
    target: &AssuranceTarget,
    case: &ContractCaseFacts,
    evaluation: &mut Evaluation,
) {
    let observation = &case.handoff.observation.value;
    let proof_acceptance = observation.get("proof_acceptance").and_then(Value::as_str);
    if !proof_acceptance.is_some_and(|relay| {
        request
            .policy
            .mantle
            .proof_acceptance_relays
            .iter()
            .any(|admitted| admitted == relay)
    }) {
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

    if let Err(admission) = admit_producer_job(&request.policy, &case.handoff.job.value) {
        let rank = match admission.code {
            CONVERGENCE_ISSUE_UNKNOWN_PRODUCER_OWNER => Rank::Unsupported,
            CONVERGENCE_ISSUE_PRODUCER_SCHEMA_MISMATCH => Rank::Stale,
            _ => Rank::Incomplete,
        };
        evaluation.mark(rank, admission.code);
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
        node_id: None,
        target_derivation: None,
        target_path: None,
        nar_sha256: None,
        root_disposition: None,
        original_receipt_blake3: None,
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

    if value.get("schema").and_then(Value::as_str) == Some(REALIZATION_RECEIPT_SCHEMA_V2) {
        check_realization_v2(request, case, value, &mut binding, evaluation);
    } else {
        check_realization_v1(case, value, &mut binding, evaluation);
    }
    binding
}

/// Version 1 receipts keep their existing binding: the unit's node_id and the
/// output whose path the bridge consumed. Their disposition is derived, never
/// read: the preserve route did not build, the version 1 build route always did.
fn check_realization_v1(
    case: &ContractCaseFacts,
    value: &Value,
    binding: &mut RealizationBinding,
    evaluation: &mut Evaluation,
) {
    let expected_node_id = expected_root_node_id(case);
    let unit = value
        .get("units")
        .and_then(Value::as_array)
        .and_then(|units| {
            units.iter().find(|unit| {
                unit.get("node_id").and_then(Value::as_str) == Some(expected_node_id.as_str())
            })
        });
    let Some(unit) = unit else {
        evaluation.mark(
            Rank::Stale,
            CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH,
        );
        return;
    };
    binding.node_id = unit
        .get("node_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.target_derivation = unit
        .get("target_derivation")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
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
        return;
    };
    binding.target_path = output
        .get("target_path")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.nar_sha256 = output
        .get("nar_sha256")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.root_disposition = v1_root_disposition(value).map(ToOwned::to_owned);
}

/// Version 2 receipts name each selected root. Cairn admits a root only when
/// the policy admits its disposition, its signer has the builder role and a
/// fingerprint the case trusts, and a `reused` root links a `built` original.
///
/// r[impl cairn.convergence.realization.root_disposition]
/// r[impl cairn.convergence.realization.reuse_link]
fn check_realization_v2(
    request: &ConvergenceRequest,
    case: &ContractCaseFacts,
    value: &Value,
    binding: &mut RealizationBinding,
    evaluation: &mut Evaluation,
) {
    let expected_node_id = expected_root_node_id(case);
    let dir_name = case.producer_output.dir_name.as_str();
    let roots = value.get("roots").and_then(Value::as_array);
    let node_roots: Vec<&Value> = roots
        .map(|roots| {
            roots
                .iter()
                .filter(|root| {
                    root.get("node_id").and_then(Value::as_str) == Some(expected_node_id.as_str())
                })
                .collect()
        })
        .unwrap_or_default();
    if node_roots.is_empty() {
        evaluation.mark(
            Rank::Stale,
            CONVERGENCE_ISSUE_REALIZATION_DERIVATION_MISMATCH,
        );
        return;
    }
    let root = node_roots.iter().copied().find(|root| {
        root.get("path")
            .and_then(Value::as_str)
            .map(final_component)
            == Some(dir_name)
    });
    let Some(root) = root else {
        evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REALIZATION_OUTPUT_MISMATCH);
        return;
    };
    binding.node_id = root
        .get("node_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.target_derivation = root
        .get("target_derivation")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.target_path = root
        .get("path")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    binding.nar_sha256 = root
        .get("nar_sha256")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let disposition = root.get("disposition").and_then(Value::as_str);
    let Some(disposition) = disposition.filter(|disposition| {
        request
            .policy
            .mantle
            .root_dispositions
            .iter()
            .any(|admitted| admitted == disposition)
    }) else {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE);
        return;
    };
    binding.root_disposition = Some(disposition.to_string());

    let built = root.get("built").filter(|value| !value.is_null());
    let reused = root.get("reused").filter(|value| !value.is_null());
    let shape_ok = match disposition {
        ROOT_DISPOSITION_BUILT => built.is_some() && reused.is_none(),
        ROOT_DISPOSITION_REUSED => reused.is_some() && built.is_none(),
        _ => false,
    };
    if !shape_ok {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_REALIZATION_INCOMPLETE);
    }

    let signer = root.get("signer");
    let role_is_builder = signer
        .and_then(|signer| signer.get("role"))
        .and_then(Value::as_str)
        == Some(BUILDER_SIGNER_ROLE);
    let fingerprint = signer_fingerprint(root);
    let fingerprint_trusted = fingerprint.is_some_and(|fingerprint| {
        request
            .trusted_builder_keys
            .iter()
            .any(|trusted| trusted == fingerprint)
    });
    if !(role_is_builder && fingerprint_trusted) {
        evaluation.mark(Rank::Unrealized, CONVERGENCE_ISSUE_UNTRUSTED_SIGNER);
    }

    if disposition == ROOT_DISPOSITION_REUSED {
        let link = reused
            .and_then(|reused| reused.get("original_receipt_blake3"))
            .and_then(Value::as_str);
        let Some(link) = link else {
            evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REUSE_LINK_INVALID);
            return;
        };
        binding.original_receipt_blake3 = Some(link.to_string());
        if !reuse_link_is_valid(root, link, case.original_realization.as_ref()) {
            evaluation.mark(Rank::Stale, CONVERGENCE_ISSUE_REUSE_LINK_INVALID);
        }
    }
}

/// A version 2 `reused` root must link a parseable version 2 original whose
/// recorded digest is the link, whose matching root was `built` with the same
/// NAR hash, and whose signer fingerprint is the same.
///
/// Cairn checks the recorded identities; it does not verify signature bytes.
fn reuse_link_is_valid(root: &Value, link: &str, original: Option<&FileFacts>) -> bool {
    let Some(original) = original else {
        return false;
    };
    let original_value = &original.value;
    if original_value.get("schema").and_then(Value::as_str) != Some(REALIZATION_RECEIPT_SCHEMA_V2) {
        return false;
    }
    if original_value.get("receipt_blake3").and_then(Value::as_str) != Some(link) {
        return false;
    }
    let original_root = original_value
        .get("roots")
        .and_then(Value::as_array)
        .and_then(|roots| {
            roots.iter().find(|candidate| {
                candidate.get("node_id") == root.get("node_id")
                    && candidate.get("target_derivation") == root.get("target_derivation")
                    && candidate.get("output_name") == root.get("output_name")
                    && candidate.get("path") == root.get("path")
            })
        });
    let Some(original_root) = original_root else {
        return false;
    };
    if original_root.get("disposition").and_then(Value::as_str) != Some(ROOT_DISPOSITION_BUILT) {
        return false;
    }
    if original_root.get("built").is_none_or(Value::is_null)
        || original_root
            .get("reused")
            .is_some_and(|value| !value.is_null())
    {
        return false;
    }
    let same_nar = match (
        root.get("nar_sha256").and_then(Value::as_str),
        original_root.get("nar_sha256").and_then(Value::as_str),
    ) {
        (Some(current), Some(original)) => digest_eq(current, original),
        _ => false,
    };
    same_nar
        && matches!(
            (signer_fingerprint(root), signer_fingerprint(original_root)),
            (Some(current), Some(original)) if current == original
        )
}

/// The version 1 route's meaning for `bound.root_disposition`: the preserve
/// route built nothing, and the version 1 build route always built. Any other
/// route, including none, makes no claim.
fn v1_root_disposition(value: &Value) -> Option<&'static str> {
    match value
        .pointer("/cache_policy/realization_route")
        .and_then(Value::as_str)
    {
        Some(CACHE_ONLY_PRESERVE_ROUTE) => Some(ROOT_DISPOSITION_PRESERVED),
        Some(CACHE_INPUTS_BUILD_ROOTS_ROUTE_V1) => Some(ROOT_DISPOSITION_BUILT),
        _ => None,
    }
}

fn signer_fingerprint(root: &Value) -> Option<&str> {
    root.pointer("/signer/public_key_blake3")
        .and_then(Value::as_str)
}

/// Mantle names a foreign unit `nix:` plus the foreign derivation's file name;
/// `target_derivation` is Mantle's translated derivation, which differs from the
/// frozen root even on the preserve route.
fn expected_root_node_id(case: &ContractCaseFacts) -> String {
    let root_derivation = case
        .handoff
        .root_derivation
        .text
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    format!("nix:{}", final_component(root_derivation))
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
