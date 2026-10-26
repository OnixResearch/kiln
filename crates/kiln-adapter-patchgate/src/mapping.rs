use std::collections::{BTreeMap, BTreeSet};

use kiln_core::provider::{Operation, OperationRequest, ProviderRole, admit_operation};
use kiln_core::review::{
    ChangeReview, ReviewDecision, ReviewEffectPlan, ReviewEvaluationObservation,
    admit_review_observation, validate_review_effect_plan,
};
use kiln_core::types::CONTRACT_VERSION_V1;
use kiln_provider_api::{PortFailure, PortResult};
use serde::Deserialize;

use crate::profile::{canonical_identity, native_hash, sha256, validate_main, validate_profile};
use crate::{
    ConfigurationEvidence, MainBinding, PatchgateProfile, PolicyBinding, candidate_root,
    configuration_identity, domain, failure, native_binding_identity, policy_identity,
};

#[derive(Deserialize)]
struct Report {
    schema_version: u16,
    dry_run: bool,
    authorizes_acceptance: bool,
    main: MainBinding,
    inbox: MainBinding,
    configuration: ConfigurationEvidence,
    selection: Selection,
    items: Vec<Item>,
    records: Vec<Record>,
    review_usage: Option<ReviewUsage>,
}

#[derive(Deserialize)]
struct Selection {
    requested_changes: Vec<String>,
    requested_authors: Vec<String>,
    include_dependencies: bool,
    selected: Vec<String>,
    added_dependencies: Vec<String>,
}

#[derive(Deserialize)]
struct Item {
    hash: String,
    state: ReviewDecision,
    needs: Vec<(String, ReviewDecision)>,
    decided: bool,
    merge: Option<Record>,
}

#[derive(Deserialize)]
struct Record {
    hash: String,
    decision: String,
    by: String,
    model: Option<String>,
    review: Option<ReviewBinding>,
    execution_error: Option<String>,
    verification: Option<Verification>,
    configuration: Option<ConfigurationEvidence>,
    coverage: Option<Coverage>,
    provenance: Option<Provenance>,
    #[serde(default, rename = "for")]
    for_hashes: Vec<String>,
}

#[derive(Deserialize)]
struct ReviewBinding {
    candidates: Vec<String>,
    policy: Option<PolicyBinding>,
    main: MainBinding,
}

#[derive(Deserialize)]
struct Verification {
    schema_version: u16,
    candidates: Vec<String>,
    main: MainBinding,
    source: StoreTree,
    definitions: StoreTree,
    inputs: BTreeMap<String, StoreTree>,
    results: Vec<CheckResult>,
}

#[derive(Deserialize)]
struct StoreTree {
    path: String,
    nar_hash: String,
}

#[derive(Deserialize)]
struct CheckResult {
    check: String,
    drv_path: String,
    outputs: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Provenance {
    requests: Vec<RequestEvidence>,
    verifiers: VerifierVersions,
}

#[derive(Deserialize)]
struct RequestEvidence {
    schema_version: u16,
    operation: String,
    api: String,
    model: String,
    revision_verified: bool,
    prompt_sha256: String,
    schema_sha256: String,
}

#[derive(Deserialize)]
struct VerifierVersions {
    schema_version: u16,
    patchgate: String,
    pijul: String,
}

#[derive(Deserialize)]
struct Coverage {
    schema_version: u16,
    planned_pages: usize,
    complete: bool,
    pages: Vec<CoveragePage>,
}

#[derive(Deserialize)]
struct CoveragePage {
    index: usize,
    decision: String,
}

#[derive(Deserialize)]
struct ReviewUsage {
    requests: usize,
    max_requests: usize,
    elapsed_seconds: f64,
    timeout_seconds: f64,
    stop: Option<ReviewStop>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ReviewStop {
    RequestLimit,
    Timeout,
    Interrupted,
}

pub(crate) fn validate_plan(
    profile: &PatchgateProfile,
    plan: &ReviewEffectPlan,
    operation: Operation,
) -> PortResult<()> {
    validate_profile(profile)?;
    validate_review_effect_plan(plan, profile.limits.max_changes, profile.limits.max_edges)
        .map_err(PortFailure::Domain)?;
    let (encoded, _) = kiln_codec::encode_and_identify(plan, profile.limits.max_request_bytes)
        .map_err(|_| {
            domain(
                "patchgate_request_bound",
                "Review request exceeds its canonical byte bound",
            )
        })?;
    admit_operation(
        &profile.provider,
        &OperationRequest {
            contract_version: CONTRACT_VERSION_V1,
            selected_profile: plan.provider_profile.clone(),
            role: ProviderRole::Collaboration,
            operation,
            request_bytes: encoded.len(),
            required_outcomes: BTreeSet::new(),
        },
    )
    .map_err(PortFailure::Domain)?;
    if candidate_root(&plan.candidate, profile.limits.max_request_bytes)? != plan.candidate_root
        || native_binding_identity(profile)? != plan.native_binding_identity
        || policy_identity(profile)? != plan.policy_identity
    {
        return Err(domain(
            "patchgate_request_identity",
            "Candidate, native execution binding or policy identity differs from the exact request",
        ));
    }
    for candidate in &plan.candidate.ordered_changes {
        if candidate.change.provider_namespace.as_str() != "pijul"
            || candidate.change.native_kind.as_str() != "change"
            || !native_hash(candidate.change.native_id.as_str())
            || candidate.target_revision.provider_namespace.as_str() != "pijul"
            || candidate.target_revision.native_kind.as_str() != "state"
            || candidate.target_revision.native_id.as_str()
                != profile.native.main.state.as_deref().unwrap_or("empty")
        {
            return Err(domain(
                "patchgate_native_candidate",
                "Candidate requires explicit full Pijul change hashes and the bound initial main state",
            ));
        }
    }
    Ok(())
}

pub(crate) fn admit_report(
    profile: &PatchgateProfile,
    plan: &ReviewEffectPlan,
    raw: &[u8],
) -> PortResult<ReviewEvaluationObservation> {
    if raw.is_empty() || raw.len() > profile.limits.max_report_bytes {
        return Err(domain(
            "patchgate_report_bound",
            "Native report is empty or exceeds its named byte bound",
        ));
    }
    let report: Report = serde_json::from_slice(raw).map_err(|_| {
        domain(
            "patchgate_report_json",
            "Native output is not one complete supported report",
        )
    })?;
    if report.schema_version != profile.report_schema
        || !report.dry_run
        || report.authorizes_acceptance
    {
        return Err(domain(
            "patchgate_report_contract",
            "Native report must be schema 1, isolated dry-run and non-authorizing",
        ));
    }
    if report.main != profile.native.main
        || report.inbox != profile.native.inbox
        || configuration_identity(&report.configuration, profile.limits.max_report_bytes)?
            != configuration_identity(
                &profile.native.configuration,
                profile.limits.max_request_bytes,
            )?
    {
        return Err(domain(
            "patchgate_initial_binding",
            "Native initial channels, states or trusted configuration differ from the request",
        ));
    }
    if let Some(usage) = &report.review_usage {
        if usage.max_requests != profile.native.configuration.values.max_review_requests
            || usage.timeout_seconds != profile.native.configuration.values.review_timeout_seconds
            || usage.requests > usage.max_requests
            || !usage.elapsed_seconds.is_finite()
            || usage.elapsed_seconds < 0.0
        {
            return Err(domain(
                "patchgate_review_usage",
                "Native review usage differs from trusted review limits",
            ));
        }
        match usage.stop {
            Some(ReviewStop::Timeout) => {
                return Err(PortFailure::TimedOut(failure(
                    "patchgate_review_timeout",
                    "Native review stopped at its deadline",
                )));
            }
            Some(ReviewStop::Interrupted) => {
                return Err(PortFailure::Cancelled(failure(
                    "patchgate_review_interrupted",
                    "Native review was interrupted",
                )));
            }
            Some(ReviewStop::RequestLimit) => {
                return Err(PortFailure::Infrastructure(failure(
                    "patchgate_review_request_limit",
                    "Native review stopped at its request limit",
                )));
            }
            None => {}
        }
    }
    let expected: BTreeSet<&str> = plan
        .candidate
        .ordered_changes
        .iter()
        .map(|candidate| candidate.change.native_id.as_str())
        .collect();
    let selection = &report.selection;
    if !exact_set(&selection.requested_changes, &expected)
        || !exact_set(&selection.selected, &expected)
        || !selection.requested_authors.is_empty()
        || selection.include_dependencies
        || !selection.added_dependencies.is_empty()
    {
        return Err(domain(
            "patchgate_selection",
            "Native requested and selected sets must exactly equal explicit candidates without added prerequisites or author selectors",
        ));
    }
    let mut items = BTreeMap::new();
    for item in &report.items {
        if !expected.contains(item.hash.as_str())
            || items.insert(item.hash.as_str(), item).is_some()
            || item
                .needs
                .iter()
                .any(|(hash, _)| !expected.contains(hash.as_str()))
        {
            return Err(domain(
                "patchgate_items",
                "Native items contain a duplicate, unrelated change or undeclared prerequisite",
            ));
        }
    }
    if items.len() != expected.len() {
        return Err(domain(
            "patchgate_items",
            "Native report is missing selected change results",
        ));
    }
    let mut records = BTreeMap::new();
    for record in &report.records {
        validate_record(profile, record, &expected)?;
        if record.decision == "merge" {
            if expected.contains(record.hash.as_str())
                || record.for_hashes.is_empty()
                || !subset(&record.for_hashes, &expected)
            {
                return Err(domain(
                    "patchgate_merge",
                    "Native synthetic merge must refer only to explicit candidates",
                ));
            }
        } else if !expected.contains(record.hash.as_str()) {
            return Err(domain(
                "patchgate_records",
                "Native decision record refers to an unrelated change",
            ));
        }
        if records.insert(record.hash.as_str(), record).is_some() {
            return Err(domain(
                "patchgate_records",
                "Native report duplicates a change decision record",
            ));
        }
    }
    let mut reviews = Vec::with_capacity(plan.candidate.ordered_changes.len());
    for candidate in &plan.candidate.ordered_changes {
        let hash = candidate.change.native_id.as_str();
        let item = items
            .get(hash)
            .ok_or_else(|| domain("patchgate_items", "Selected result is missing"))?;
        let record = records.get(hash).copied();
        if matches!(
            item.state,
            ReviewDecision::Accepted | ReviewDecision::Rejected | ReviewDecision::Escalated
        ) && (!item.decided || record.is_none())
        {
            return Err(domain(
                "patchgate_decision_evidence",
                "Terminal review decisions require a fresh decided item and supporting record; historical state is not a new review",
            ));
        }
        if let Some(record) = record {
            let consistent = match record.decision.as_str() {
                "accept" => matches!(
                    item.state,
                    ReviewDecision::Accepted | ReviewDecision::Removed
                ),
                "reject" => item.state == ReviewDecision::Rejected,
                "escalate" => item.state == ReviewDecision::Escalated,
                "revert" => item.state == ReviewDecision::Reverted,
                _ => item.state == ReviewDecision::Unknown,
            };
            if !consistent {
                return Err(domain(
                    "patchgate_decision_conflict",
                    "Reported native item state conflicts with its decision record",
                ));
            }
        }
        if let Some(merge) = &item.merge {
            validate_record(profile, merge, &expected)?;
            if merge.decision != "merge"
                || !merge.for_hashes.iter().any(|target| target == hash)
                || !subset(&merge.for_hashes, &expected)
                || !records.contains_key(merge.hash.as_str())
            {
                return Err(domain(
                    "patchgate_merge",
                    "Selected result has an unbound synthetic merge",
                ));
            }
        }
        reviews.push(ChangeReview {
            change: candidate.change.clone(),
            decision: item.state,
        });
    }
    let observation = ReviewEvaluationObservation {
        contract_version: CONTRACT_VERSION_V1,
        effect_id: plan.effect_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate.id.clone(),
        candidate_root: plan.candidate_root.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        native_binding_identity: plan.native_binding_identity.clone(),
        report_identity: kiln_codec::identity_from_bytes(raw).map_err(|_| {
            domain(
                "patchgate_report_identity",
                "Cannot identify native report bytes",
            )
        })?,
        isolated: true,
        authorizes_acceptance: false,
        reviews,
    };
    admit_review_observation(
        plan,
        &observation,
        profile.limits.max_changes,
        profile.limits.max_edges,
    )
    .map_err(PortFailure::Domain)?;
    Ok(observation)
}

fn exact_set(values: &[String], expected: &BTreeSet<&str>) -> bool {
    values.len() == expected.len() && subset(values, expected)
}

fn subset(values: &[String], expected: &BTreeSet<&str>) -> bool {
    let unique: BTreeSet<&str> = values.iter().map(String::as_str).collect();
    unique.len() == values.len() && unique.is_subset(expected)
}

fn validate_record(
    profile: &PatchgateProfile,
    record: &Record,
    expected: &BTreeSet<&str>,
) -> PortResult<()> {
    if !native_hash(&record.hash) || record.by.is_empty() {
        return Err(domain(
            "patchgate_record",
            "Native decision record has no full hash or actor",
        ));
    }
    if record.execution_error.is_some() {
        return Err(PortFailure::Infrastructure(failure(
            "patchgate_native_execution",
            "Native record reports an execution error, not a completed review decision",
        )));
    }
    if let Some(provenance) = &record.provenance {
        if provenance.verifiers.schema_version != 1
            || provenance.verifiers.patchgate.is_empty()
            || provenance.verifiers.pijul.is_empty()
            || provenance.requests.iter().any(|request| {
                request.schema_version != 1
                    || request.operation.is_empty()
                    || request.api.is_empty()
                    || request.model.is_empty()
                    || request.revision_verified
                    || !sha256(&request.prompt_sha256)
                    || !sha256(&request.schema_sha256)
            })
        {
            return Err(domain(
                "patchgate_provenance",
                "Native provenance is incomplete or claims unsupported model revision verification",
            ));
        }
    }
    if let Some(configuration) = &record.configuration {
        if canonical_identity(configuration, profile.limits.max_report_bytes)?
            != canonical_identity(
                &profile.native.configuration,
                profile.limits.max_request_bytes,
            )?
        {
            return Err(domain(
                "patchgate_record_configuration",
                "Native decision configuration differs from trusted configuration",
            ));
        }
    }
    if let Some(binding) = &record.review {
        validate_record_main(profile, &binding.main)?;
        if binding.policy != profile.native.policy
            || binding.candidates.is_empty()
            || !subset(&binding.candidates, expected)
            || (record.decision != "merge" && !binding.candidates.contains(&record.hash))
        {
            return Err(domain(
                "patchgate_record_binding",
                "Native review binds foreign candidates or a different native policy",
            ));
        }
    }
    if let Some(verification) = &record.verification {
        validate_record_main(profile, &verification.main)?;
        if verification.schema_version != 1
            || verification.candidates.is_empty()
            || !subset(&verification.candidates, expected)
            || (record.decision != "merge" && !verification.candidates.contains(&record.hash))
            || !valid_tree(&verification.source)
            || !valid_tree(&verification.definitions)
            || verification.inputs.values().any(|tree| !valid_tree(tree))
            || verification.results.is_empty()
            || verification.results.iter().any(|result| {
                result.check.is_empty()
                    || !store_path(&result.drv_path)
                    || result.outputs.is_empty()
                    || result.outputs.values().any(|path| !store_path(path))
            })
        {
            return Err(domain(
                "patchgate_verification",
                "Native verification has foreign candidates or unsupported schema",
            ));
        }
    }
    if let Some(coverage) = &record.coverage {
        let approving = matches!(record.decision.as_str(), "accept" | "merge");
        if coverage.schema_version != 1
            || coverage.planned_pages == 0
            || coverage.pages.len() > coverage.planned_pages
            || coverage.pages.iter().enumerate().any(|(index, page)| {
                page.index != index
                    || !matches!(page.decision.as_str(), "accept" | "reject" | "escalate")
            })
            || (approving
                && (!coverage.complete
                    || coverage.planned_pages != coverage.pages.len()
                    || coverage.pages.iter().any(|page| page.decision != "accept")))
        {
            return Err(domain(
                "patchgate_completion",
                "Native contextual review coverage is invalid or cannot support acceptance",
            ));
        }
    }
    if record.by == "model"
        && (record.model.as_ref().is_none_or(String::is_empty)
            || record.review.is_none()
            || record.provenance.is_none())
    {
        return Err(domain(
            "patchgate_model_evidence",
            "Model decision lacks its native model, review binding or provenance",
        ));
    }
    // Deterministic gate acceptance intentionally needs no model or review binding.
    Ok(())
}

fn store_path(value: &str) -> bool {
    value.starts_with("/nix/store/") && !value.chars().any(char::is_control)
}

fn valid_tree(tree: &StoreTree) -> bool {
    store_path(&tree.path)
        && tree.nar_hash.strip_prefix("sha256-").is_some_and(|hash| {
            hash.len() == 44
                && hash.ends_with('=')
                && hash.as_bytes().get(..43).is_some_and(|bytes| {
                    bytes
                        .iter()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
                })
        })
}

fn validate_record_main(profile: &PatchgateProfile, main: &MainBinding) -> PortResult<()> {
    validate_main(main)?;
    if main.channel != profile.native.main.channel {
        return Err(domain(
            "patchgate_record_main",
            "Native record refers to a different main channel",
        ));
    }
    // Each simulated acceptance can advance main. Preserve its actual per-record state.
    Ok(())
}
