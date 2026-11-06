use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
    BuildMetricObservation, BuildProductObservation, admit_build_evaluation,
    validate_build_effect_plan,
};
use kiln_core::types::{Alias, ContentId, Diagnostic};
use kiln_provider_api::{PortFailure, PortResult};
use serde::Deserialize;

use crate::profile::MantleProfile;
use crate::wire::{
    BuildObservation, BuildOutcome, BuildRequest, CacheKind, CacheObservation, Identity, Label,
    MetricObservation, NATIVE_REPORT_SCHEMA, OBSERVATION_SCHEMA, ProductObservation,
    REQUEST_SCHEMA, REQUIRED_NON_CLAIMS, observation_identity, request_identity,
    validate_observation, validate_request,
};

const EFFECT_IDENTITY_DOMAIN: &str = "kiln.mantle.effect-identity.v1";
const ATTEMPT_IDENTITY_DOMAIN: &str = "kiln.mantle.attempt-identity.v1";
const CANDIDATE_IDENTITY_DOMAIN: &str = "kiln.mantle.candidate-identity.v1";
const PIPELINE_IDENTITY_DOMAIN: &str = "kiln.mantle.pipeline-identity.v1";
const ARTIFACT_IDENTITY_DOMAIN: &str = "kiln.mantle.artifact-reference.v1";
const STORE_OBJECT_IDENTITY_DOMAIN: &str = "kiln.mantle.store-object-reference.v1";
const LOG_IDENTITY_DOMAIN: &str = "kiln.mantle.log-reference.v1";
const REPORT_RECEIPT_DOMAIN: &str = "kiln.mantle.native-report-receipt.v1";
const SHELL_RECEIPT_DOMAIN: &str = "kiln.mantle.shell-receipt.v1";
const NATIVE_FAILURE_PHASES: [&str; 2] = ["preflight", "build"];
const NATIVE_FAILURE_CLASSES: [&str; 8] = [
    "preflight",
    "fixed-output-hash-mismatch",
    "sandbox",
    "missing-output",
    "missing-source-input",
    "cache-verification",
    "unsupported-platform",
    "builder",
];
const HERMETICITY_MODES: [&str; 3] = ["practical", "strict", "impure"];
pub(crate) const NATIVE_REPORT_METRIC_COUNT: usize = 39;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedBuild {
    pub observation: BuildEvaluationObservation,
    pub logs: BTreeMap<ContentId, PathBuf>,
    pub product_paths: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeBuildReport {
    schema: String,
    file: String,
    output_dir: String,
    state_dir: String,
    store_dir: String,
    scheduler_policy: NativeSchedulingPolicy,
    hermeticity_mode: String,
    hermeticity_audit_events: Vec<serde_json::Value>,
    build_environment_reports: Vec<serde_json::Value>,
    #[serde(default)]
    finish_gates: Vec<serde_json::Value>,
    network_policy_reports: Vec<serde_json::Value>,
    workspace_reports: Vec<serde_json::Value>,
    #[serde(default)]
    action_result_reports: Vec<serde_json::Value>,
    native_dynamic_plans: Vec<serde_json::Value>,
    #[serde(default)]
    plan_output_bindings: Vec<serde_json::Value>,
    scheduler_priority_decisions: Vec<serde_json::Value>,
    overlay_base_generations: Vec<serde_json::Value>,
    #[serde(default)]
    overlay_plan_blake3: Option<String>,
    store_layer_selections: Vec<serde_json::Value>,
    #[serde(default)]
    remote_telemetry_events: Vec<serde_json::Value>,
    #[serde(default)]
    remote_observability: Option<Vec<serde_json::Value>>,
    frontend_artifact_attestations: Vec<serde_json::Value>,
    ast_grep_structural_evidence: Vec<serde_json::Value>,
    ast_grep_structural_evidence_diagnostics: Vec<serde_json::Value>,
    cargo_build_evidence: Vec<serde_json::Value>,
    cargo_build_evidence_diagnostics: Vec<serde_json::Value>,
    diagnostic_persistence_failures: Vec<serde_json::Value>,
    counts: NativeCounts,
    outcomes: Vec<NativeOutcome>,
    failed: Vec<NativeFailure>,
    fod_mismatches: Vec<NativeFodMismatch>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSchedulingPolicy {
    schema: String,
    policy_id: String,
    preference_order: Vec<String>,
    aged_after_epochs: u64,
    protected_after_epochs: u64,
}

#[derive(Debug, Deserialize)]
struct NativeCounts {
    succeeded_total: u32,
    built_total: u32,
    cached_total: u32,
    failed_total: u32,
}

#[derive(Debug, Deserialize)]
struct NativeOutcome {
    drv_key: String,
    label: String,
    cached: bool,
    log_file: Option<String>,
    outputs: Vec<NativeOutput>,
}

#[derive(Debug, Deserialize)]
struct NativeOutput {
    name: String,
    path: String,
    artifact_attestation: NativeAttestation,
    substitution: Option<NativeSubstitution>,
    cache_admission: Option<NativeCacheAdmission>,
}

#[derive(Debug, Deserialize)]
struct NativeAttestation {
    logical_path: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct NativeSubstitution {
    mode: String,
    transferred_bytes: u64,
    reused_bytes: u64,
    fallback_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NativeCacheAdmission {
    reason: String,
}

#[derive(Debug, Deserialize)]
struct NativeFailure {
    root: String,
    drv_key: String,
    phase: String,
    error_class: String,
    message: String,
    saved_log_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NativeFodMismatch {
    name: String,
    expected_sri: String,
    actual_sri: String,
}

// r[impl kiln.mantle.request]
pub(crate) fn lower_request(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
) -> PortResult<BuildRequest> {
    validate_build_effect_plan(plan, profile.limits.max_required_products)?;
    if plan.provider_profile != profile.provider.profile_id
        || plan.plan_identity != profile.runtime.plan_identity
        || plan.policy_identity != profile.runtime.policy_identity
        || plan.platform.as_str() != profile.platform
    {
        return Err(domain(
            "mantle_request_profile",
            "mantle.request",
            "Mantle build plan differs from the admitted provider, plan, policy, or platform",
        ));
    }
    let mut request = BuildRequest {
        schema: REQUEST_SCHEMA.to_string(),
        identity: identity_from_content(&plan.idempotency_key)?,
        idempotency_key: identity_from_content(&plan.idempotency_key)?,
        effect_identity: text_identity(EFFECT_IDENTITY_DOMAIN, plan.effect_id.as_str())?,
        attempt_identity: text_identity(ATTEMPT_IDENTITY_DOMAIN, plan.attempt_id.as_str())?,
        candidate_identity: text_identity(CANDIDATE_IDENTITY_DOMAIN, plan.candidate_id.as_str())?,
        pipeline_revision_identity: text_identity(
            PIPELINE_IDENTITY_DOMAIN,
            plan.pipeline_revision.as_str(),
        )?,
        plan_identity: identity_from_content(&plan.plan_identity)?,
        policy_identity: identity_from_content(&plan.policy_identity)?,
        platform: label(plan.platform.as_str(), "mantle.request.platform")?,
        required_products: plan
            .required_products
            .iter()
            .map(|value| label(value.as_str(), "mantle.request.required_products"))
            .collect::<PortResult<Vec<_>>>()?,
    };
    request.identity = request_identity(&request);
    validate_request(&request).map_err(contract_error)?;
    Ok(request)
}

// r[impl kiln.mantle.completion]
// r[impl kiln.mantle.native_report.surface]
// r[impl kiln.mantle.native_report.facts]
// r[impl kiln.mantle.native_report.facts.nonclaims]
pub(crate) fn normalize_native_report(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    bytes: &[u8],
) -> PortResult<NormalizedBuild> {
    let report: NativeBuildReport = serde_json::from_slice(bytes).map_err(|_| {
        domain(
            "mantle_native_report_parse",
            "mantle.report",
            "Mantle native report is not valid bounded JSON",
        )
    })?;
    validate_report_surface(&report)?;
    validate_report_binding(profile, &report)?;
    let receipt = tagged_identity(REPORT_RECEIPT_DOMAIN, &[bytes])?;
    let required = request
        .required_products
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();
    let mut products = BTreeMap::<String, ProductObservation>::new();
    let mut product_paths = BTreeMap::<String, PathBuf>::new();
    let mut logs = BTreeMap::<ContentId, PathBuf>::new();
    let mut saw_cached = false;
    let mut saw_substitution = false;
    let mut cache_admissions = 0_u64;
    let mut substitution_outputs = 0_u64;
    let mut substitution_transferred_bytes = 0_u64;
    let mut substitution_reused_bytes = 0_u64;
    let mut substitution_fallbacks = 0_u64;
    let mut total_outputs = 0_usize;
    for outcome in &report.outcomes {
        require_non_empty(
            &outcome.drv_key,
            "mantle.report.outcomes.drv_key",
            "Mantle native outcome is missing its derivation key",
        )?;
        require_non_empty(
            &outcome.label,
            "mantle.report.outcomes.label",
            "Mantle native outcome is missing its label",
        )?;
        saw_cached |= outcome.cached;
        if let Some(path) = &outcome.log_file {
            insert_log(
                &mut logs,
                &receipt,
                path,
                "mantle.report.outcomes.log_file",
                &profile.runtime.state_path,
            )?;
        }
        let mut outcome_outputs = BTreeSet::new();
        for output in &outcome.outputs {
            total_outputs = total_outputs.checked_add(1).ok_or_else(|| {
                domain(
                    "mantle_report_bound",
                    "mantle.report.outcomes.outputs",
                    "Mantle native report output count is not representable",
                )
            })?;
            require_non_empty(
                &output.name,
                "mantle.report.outcomes.outputs.name",
                "Mantle native output is missing its name",
            )?;
            if !outcome_outputs.insert(output.name.as_str()) {
                return Err(domain(
                    "mantle_product_duplicate",
                    "mantle.report.outcomes.outputs",
                    "Mantle native outcome contains a duplicate output",
                ));
            }
            require_path_under(
                &output.path,
                &profile.runtime.store_path,
                "mantle.report.outcomes.outputs.path",
                "Mantle product path must be inside the physical store",
            )?;
            require_path_under(
                &output.artifact_attestation.logical_path,
                &profile.runtime.store_prefix,
                "mantle.report.outcomes.outputs.artifact_attestation.logical_path",
                "Mantle artifact logical path must be inside the logical store prefix",
            )?;
            require_path_under(
                &output.artifact_attestation.path,
                &profile.runtime.state_path,
                "mantle.report.outcomes.outputs.artifact_attestation.path",
                "Mantle artifact path must be inside the state directory",
            )?;
            if let Some(admission) = &output.cache_admission {
                require_non_empty(
                    &admission.reason,
                    "mantle.report.outcomes.outputs.cache_admission.reason",
                    "Mantle cache admission reason must not be empty",
                )?;
                checked_increment(
                    &mut cache_admissions,
                    1,
                    "mantle.report.outcomes.outputs.cache_admission",
                )?;
            }
            if let Some(substitution) = &output.substitution {
                validate_substitution(substitution)?;
                saw_substitution = true;
                checked_increment(
                    &mut substitution_outputs,
                    1,
                    "mantle.report.outcomes.outputs.substitution",
                )?;
                checked_add(
                    &mut substitution_transferred_bytes,
                    substitution.transferred_bytes,
                    "mantle.report.outcomes.outputs.substitution.transferred_bytes",
                )?;
                checked_add(
                    &mut substitution_reused_bytes,
                    substitution.reused_bytes,
                    "mantle.report.outcomes.outputs.substitution.reused_bytes",
                )?;
                if substitution.fallback_reason.is_some() {
                    checked_increment(
                        &mut substitution_fallbacks,
                        1,
                        "mantle.report.outcomes.outputs.substitution.fallback_reason",
                    )?;
                }
            }
            if !required.contains(output.name.as_str()) {
                continue;
            }
            if products.contains_key(&output.name) {
                return Err(domain(
                    "mantle_product_duplicate",
                    "mantle.report.outcomes.outputs",
                    "Mantle native report contains a duplicate required product",
                ));
            }
            product_paths.insert(output.name.clone(), PathBuf::from(&output.path));
            products.insert(
                output.name.clone(),
                ProductObservation {
                    name: label(&output.name, "mantle.report.product.name")?,
                    artifact_identity: tagged_identity(
                        ARTIFACT_IDENTITY_DOMAIN,
                        &[
                            output.artifact_attestation.logical_path.as_bytes(),
                            output.artifact_attestation.path.as_bytes(),
                        ],
                    )?,
                    store_identity: tagged_identity(
                        STORE_OBJECT_IDENTITY_DOMAIN,
                        &[output.path.as_bytes()],
                    )?,
                },
            );
        }
    }
    for failure in &report.failed {
        validate_failure(failure)?;
        if let Some(path) = &failure.saved_log_path {
            insert_log(
                &mut logs,
                &receipt,
                path,
                "mantle.report.failed.saved_log_path",
                &profile.runtime.state_path,
            )?;
        }
    }
    for mismatch in &report.fod_mismatches {
        require_non_empty(
            &mismatch.name,
            "mantle.report.fod_mismatches.name",
            "Mantle FOD mismatch is missing its output name",
        )?;
        require_non_empty(
            &mismatch.expected_sri,
            "mantle.report.fod_mismatches.expected_sri",
            "Mantle FOD mismatch is missing its expected SRI",
        )?;
        require_non_empty(
            &mismatch.actual_sri,
            "mantle.report.fod_mismatches.actual_sri",
            "Mantle FOD mismatch is missing its actual SRI",
        )?;
    }
    validate_counts(&report)?;
    if !report.fod_mismatches.is_empty() && report.failed.is_empty() {
        return Err(domain(
            "mantle_native_report_outcome",
            "mantle.report.fod_mismatches",
            "Mantle native report has a FOD mismatch without a failure envelope",
        ));
    }
    if products.len() > profile.limits.max_products || logs.len() > profile.limits.max_logs {
        return Err(domain(
            "mantle_report_bound",
            "mantle.report",
            "Mantle native report exceeds a selected collection bound",
        ));
    }
    let successful = report.failed.is_empty();
    let outcome = if successful {
        BuildOutcome::Success
    } else {
        BuildOutcome::Failed
    };
    let product_count = products.len();
    let products = if successful {
        products.into_values().collect()
    } else {
        Vec::new()
    };
    let cache = if saw_substitution {
        CacheObservation {
            kind: CacheKind::Substitution,
            source_identity: Some(receipt.clone()),
        }
    } else if saw_cached {
        CacheObservation {
            kind: CacheKind::Local,
            source_identity: Some(identity_from_content(&profile.runtime.store_identity)?),
        }
    } else {
        CacheObservation {
            kind: CacheKind::None,
            source_identity: None,
        }
    };
    let mut metrics = Vec::with_capacity(NATIVE_REPORT_METRIC_COUNT);
    metrics.push(metric("built-total", u64::from(report.counts.built_total))?);
    metrics.push(metric(
        "cached-total",
        u64::from(report.counts.cached_total),
    )?);
    metrics.push(metric(
        "failed-total",
        u64::from(report.counts.failed_total),
    )?);
    metrics.push(metric(
        "succeeded-total",
        u64::from(report.counts.succeeded_total),
    )?);
    for (name, count) in [
        (
            "native-hermeticity-audit-events",
            report.hermeticity_audit_events.len(),
        ),
        (
            "native-build-environment-reports",
            report.build_environment_reports.len(),
        ),
        ("native-finish-gates", report.finish_gates.len()),
        (
            "native-network-policy-reports",
            report.network_policy_reports.len(),
        ),
        ("native-workspace-reports", report.workspace_reports.len()),
        (
            "native-action-result-reports",
            report.action_result_reports.len(),
        ),
        ("native-dynamic-plans", report.native_dynamic_plans.len()),
        (
            "native-plan-output-bindings",
            report.plan_output_bindings.len(),
        ),
        (
            "native-scheduler-priority-decisions",
            report.scheduler_priority_decisions.len(),
        ),
        (
            "native-overlay-base-generations",
            report.overlay_base_generations.len(),
        ),
        (
            "native-store-layer-selections",
            report.store_layer_selections.len(),
        ),
        (
            "native-remote-telemetry-events",
            report.remote_telemetry_events.len(),
        ),
        (
            "native-frontend-artifact-attestations",
            report.frontend_artifact_attestations.len(),
        ),
        (
            "native-ast-grep-structural-evidence",
            report.ast_grep_structural_evidence.len(),
        ),
        (
            "native-ast-grep-structural-evidence-diagnostics",
            report.ast_grep_structural_evidence_diagnostics.len(),
        ),
        (
            "native-cargo-build-evidence",
            report.cargo_build_evidence.len(),
        ),
        (
            "native-cargo-build-evidence-diagnostics",
            report.cargo_build_evidence_diagnostics.len(),
        ),
        (
            "native-diagnostic-persistence-failures",
            report.diagnostic_persistence_failures.len(),
        ),
        ("native-fod-mismatches", report.fod_mismatches.len()),
    ] {
        metrics.push(count_metric(name, count)?);
    }
    metrics.push(count_metric(
        "native-remote-observability",
        report.remote_observability.as_ref().map_or(0, Vec::len),
    )?);
    metrics.push(count_metric("native-output-count", total_outputs)?);
    metrics.push(count_metric("native-log-count", logs.len())?);
    metrics.push(count_metric(
        "native-required-product-count",
        product_count,
    )?);
    metrics.push(metric(
        "native-substitution-output-count",
        substitution_outputs,
    )?);
    metrics.push(metric(
        "native-substitution-transferred-bytes",
        substitution_transferred_bytes,
    )?);
    metrics.push(metric(
        "native-substitution-reused-bytes",
        substitution_reused_bytes,
    )?);
    metrics.push(metric(
        "native-substitution-fallback-count",
        substitution_fallbacks,
    )?);
    metrics.push(metric("native-cache-admission-count", cache_admissions)?);
    metrics.push(count_metric(
        "native-scheduler-preference-count",
        report.scheduler_policy.preference_order.len(),
    )?);
    metrics.push(metric(
        "native-scheduler-aged-after-epochs",
        report.scheduler_policy.aged_after_epochs,
    )?);
    metrics.push(metric(
        "native-scheduler-protected-after-epochs",
        report.scheduler_policy.protected_after_epochs,
    )?);
    metrics.push(metric(
        "native-overlay-plan-present",
        u64::from(report.overlay_plan_blake3.is_some()),
    )?);
    metrics.push(metric(
        "native-hermeticity-practical",
        u64::from(report.hermeticity_mode == "practical"),
    )?);
    metrics.push(metric(
        "native-hermeticity-strict",
        u64::from(report.hermeticity_mode == "strict"),
    )?);
    metrics.push(metric(
        "native-hermeticity-impure",
        u64::from(report.hermeticity_mode == "impure"),
    )?);
    if metrics.len() > profile.limits.max_metrics {
        return Err(domain(
            "mantle_report_bound",
            "mantle.report.metrics",
            "Mantle native report exceeds the selected metric bound",
        ));
    }
    let mut contract = BuildObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: receipt.clone(),
        request_identity: request.identity.clone(),
        outcome,
        products,
        builder_identity: identity_from_content(&profile.runtime.builder_identity)?,
        worker_identity: identity_from_content(&profile.runtime.worker_identity)?,
        store_identity: identity_from_content(&profile.runtime.store_identity)?,
        cache,
        logs: logs
            .keys()
            .map(identity_from_content)
            .collect::<PortResult<Vec<_>>>()?,
        metrics,
        receipt_identity: Some(receipt),
        non_claims: REQUIRED_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    };
    contract.identity = observation_identity(&contract);
    let observation = admitted_contract(profile, plan, request, &contract)?;
    Ok(NormalizedBuild {
        observation,
        logs,
        product_paths,
    })
}

// r[impl kiln.mantle.native_report.surface]
fn validate_report_surface(report: &NativeBuildReport) -> PortResult<()> {
    if report.schema != NATIVE_REPORT_SCHEMA {
        return Err(domain(
            "mantle_native_report_schema",
            "mantle.report.schema",
            "Mantle native report schema differs from the pinned contract",
        ));
    }
    require_non_empty(
        &report.file,
        "mantle.report.file",
        "Mantle native report is missing its source file",
    )?;
    if report.scheduler_policy.schema != "mantle-scheduling-policy-v1"
        || report.scheduler_policy.policy_id.is_empty()
        || report.scheduler_policy.preference_order.is_empty()
        || !strictly_unique(&report.scheduler_policy.preference_order)
        || report.scheduler_policy.aged_after_epochs
            > report.scheduler_policy.protected_after_epochs
    {
        return Err(domain(
            "mantle_native_report_policy",
            "mantle.report.scheduler_policy",
            "Mantle native report scheduler policy is malformed",
        ));
    }
    if !HERMETICITY_MODES.contains(&report.hermeticity_mode.as_str()) {
        return Err(domain(
            "mantle_native_report_hermeticity",
            "mantle.report.hermeticity_mode",
            "Mantle native report names an unsupported hermeticity mode",
        ));
    }
    if let Some(digest) = &report.overlay_plan_blake3 {
        if !is_blake3_hex(digest) {
            return Err(domain(
                "mantle_native_report_digest",
                "mantle.report.overlay_plan_blake3",
                "Mantle native report overlay digest is not canonical BLAKE3",
            ));
        }
    }
    Ok(())
}

// r[impl kiln.mantle.native_report.store_binding]
fn validate_report_binding(profile: &MantleProfile, report: &NativeBuildReport) -> PortResult<()> {
    if Path::new(&report.file) != profile.runtime.plan_path.as_path()
        || Path::new(&report.store_dir) != profile.runtime.store_prefix.as_path()
        || Path::new(&report.output_dir) != profile.runtime.store_path.as_path()
        || Path::new(&report.state_dir) != profile.runtime.state_path.as_path()
    {
        return Err(domain(
            "mantle_native_report_binding",
            "mantle.report",
            "Mantle native report plan or store roots differ from the profile",
        ));
    }
    Ok(())
}

// r[impl kiln.mantle.native_report.counts]
fn validate_counts(report: &NativeBuildReport) -> PortResult<()> {
    let succeeded_total = u32::try_from(report.outcomes.len()).map_err(|_| {
        domain(
            "mantle_report_bound",
            "mantle.report.counts.succeeded_total",
            "Mantle native outcome count is not representable",
        )
    })?;
    let failed_total = u32::try_from(report.failed.len()).map_err(|_| {
        domain(
            "mantle_report_bound",
            "mantle.report.counts.failed_total",
            "Mantle native failure count is not representable",
        )
    })?;
    let cached_total = u32::try_from(
        report
            .outcomes
            .iter()
            .filter(|outcome| outcome.cached)
            .count(),
    )
    .map_err(|_| {
        domain(
            "mantle_report_bound",
            "mantle.report.counts.cached_total",
            "Mantle native cached count is not representable",
        )
    })?;
    let built_total = succeeded_total.checked_sub(cached_total).ok_or_else(|| {
        domain(
            "mantle_native_report_counts",
            "mantle.report.counts",
            "Mantle native cached count exceeds the succeeded count",
        )
    })?;
    if report.counts.succeeded_total != succeeded_total
        || report.counts.built_total != built_total
        || report.counts.cached_total != cached_total
        || report.counts.failed_total != failed_total
    {
        return Err(domain(
            "mantle_native_report_counts",
            "mantle.report.counts",
            "Mantle native counts disagree with the outcome and failure arrays",
        ));
    }
    Ok(())
}

// r[impl kiln.mantle.native_report.failures]
fn validate_failure(failure: &NativeFailure) -> PortResult<()> {
    require_non_empty(
        &failure.root,
        "mantle.report.failed.root",
        "Mantle native failure is missing its root",
    )?;
    require_non_empty(
        &failure.drv_key,
        "mantle.report.failed.drv_key",
        "Mantle native failure is missing its derivation key",
    )?;
    require_non_empty(
        &failure.message,
        "mantle.report.failed.message",
        "Mantle native failure is missing its message",
    )?;
    if !NATIVE_FAILURE_PHASES.contains(&failure.phase.as_str())
        || !NATIVE_FAILURE_CLASSES.contains(&failure.error_class.as_str())
    {
        return Err(domain(
            "mantle_native_report_failure",
            "mantle.report.failed",
            "Mantle native failure phase or class is outside the pinned vocabulary",
        ));
    }
    Ok(())
}

fn validate_substitution(substitution: &NativeSubstitution) -> PortResult<()> {
    if !matches!(substitution.mode.as_str(), "delta" | "full") {
        return Err(domain(
            "mantle_native_report_substitution",
            "mantle.report.outcomes.outputs.substitution.mode",
            "Mantle native substitution mode is unsupported",
        ));
    }
    if substitution
        .fallback_reason
        .as_deref()
        .is_some_and(str::is_empty)
    {
        return Err(domain(
            "mantle_native_report_substitution",
            "mantle.report.outcomes.outputs.substitution.fallback_reason",
            "Mantle native substitution fallback reason must not be empty",
        ));
    }
    Ok(())
}

fn require_non_empty(value: &str, field: &str, message: &str) -> PortResult<()> {
    if value.is_empty() {
        Err(domain("mantle_native_report_field", field, message))
    } else {
        Ok(())
    }
}

fn strictly_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.is_empty() && seen.insert(value.as_str()))
}

// r[impl kiln.mantle.native_report.paths]
fn require_path_under(value: &str, root: &Path, field: &str, message: &str) -> PortResult<()> {
    let path = Path::new(value);
    if !path.is_absolute()
        || !path.starts_with(root)
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        Err(domain("mantle_native_report_path", field, message))
    } else {
        Ok(())
    }
}

fn insert_log(
    logs: &mut BTreeMap<ContentId, PathBuf>,
    receipt: &Identity,
    value: &str,
    field: &str,
    state_path: &Path,
) -> PortResult<()> {
    require_path_under(
        value,
        state_path,
        field,
        "Mantle log path must be inside the state directory",
    )?;
    let path = PathBuf::from(value);
    let identity = tagged_identity(
        LOG_IDENTITY_DOMAIN,
        &[
            receipt.as_str().as_bytes(),
            path.as_os_str().as_encoded_bytes(),
        ],
    )?;
    logs.insert(content_from_identity(&identity)?, path);
    Ok(())
}

fn checked_add(total: &mut u64, value: u64, field: &str) -> PortResult<()> {
    *total = total.checked_add(value).ok_or_else(|| {
        domain(
            "mantle_native_report_metric",
            field,
            "Mantle native metric total is not representable",
        )
    })?;
    Ok(())
}

fn checked_increment(total: &mut u64, value: u64, field: &str) -> PortResult<()> {
    checked_add(total, value, field)
}

fn count_metric(name: &str, value: usize) -> PortResult<MetricObservation> {
    let value = u64::try_from(value).map_err(|_| {
        domain(
            "mantle_native_report_metric",
            "mantle.report.metrics",
            "Mantle native collection count is not representable",
        )
    })?;
    metric(name, value)
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn shell_outcome(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    outcome: BuildEvaluationOutcome,
) -> PortResult<NormalizedBuild> {
    let contract_outcome = match outcome {
        BuildEvaluationOutcome::Cancelled => BuildOutcome::Cancelled,
        BuildEvaluationOutcome::TimedOut => BuildOutcome::TimedOut,
        BuildEvaluationOutcome::Unknown => BuildOutcome::Unknown,
        BuildEvaluationOutcome::Failed | BuildEvaluationOutcome::ProviderError => {
            BuildOutcome::Failed
        }
        BuildEvaluationOutcome::Success => {
            return Err(domain(
                "mantle_shell_success",
                "mantle.shell",
                "Mantle success requires one admitted native report",
            ));
        }
    };
    let receipt = if contract_outcome == BuildOutcome::Unknown {
        None
    } else {
        Some(tagged_identity(
            SHELL_RECEIPT_DOMAIN,
            &[
                request.identity.as_str().as_bytes(),
                contract_outcome.as_str().as_bytes(),
            ],
        )?)
    };
    let mut contract = BuildObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity_from_content(&plan.idempotency_key)?,
        request_identity: request.identity.clone(),
        outcome: contract_outcome,
        products: Vec::new(),
        builder_identity: identity_from_content(&profile.runtime.builder_identity)?,
        worker_identity: identity_from_content(&profile.runtime.worker_identity)?,
        store_identity: identity_from_content(&profile.runtime.store_identity)?,
        cache: CacheObservation {
            kind: CacheKind::None,
            source_identity: None,
        },
        logs: Vec::new(),
        metrics: Vec::new(),
        receipt_identity: receipt,
        non_claims: REQUIRED_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    };
    contract.identity = observation_identity(&contract);
    validate_observation(
        request,
        &identity_from_content(&profile.runtime.builder_identity)?,
        &contract,
    )
    .map_err(contract_error)?;
    let mut observation = core_observation(profile, plan, &contract)?;
    observation.outcome = outcome;
    admit_build_evaluation(plan, &observation)?;
    Ok(NormalizedBuild {
        observation,
        logs: BTreeMap::new(),
        product_paths: BTreeMap::new(),
    })
}

fn admitted_contract(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    contract: &BuildObservation,
) -> PortResult<BuildEvaluationObservation> {
    validate_observation(
        request,
        &identity_from_content(&profile.runtime.builder_identity)?,
        contract,
    )
    .map_err(contract_error)?;
    let observation = core_observation(profile, plan, contract)?;
    admit_build_evaluation(plan, &observation)?;
    Ok(observation)
}

fn core_observation(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
    contract: &BuildObservation,
) -> PortResult<BuildEvaluationObservation> {
    Ok(BuildEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        plan_identity: plan.plan_identity.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        platform: plan.platform.clone(),
        outcome: match contract.outcome {
            BuildOutcome::Success => BuildEvaluationOutcome::Success,
            BuildOutcome::Failed => BuildEvaluationOutcome::Failed,
            BuildOutcome::Cancelled => BuildEvaluationOutcome::Cancelled,
            BuildOutcome::TimedOut => BuildEvaluationOutcome::TimedOut,
            BuildOutcome::Unknown => BuildEvaluationOutcome::Unknown,
        },
        constraints: None,
        inputs: Vec::new(),
        input_closure_identity: None,
        input_closure_complete: true,
        derivation_identity: None,
        products: contract
            .products
            .iter()
            .map(|product| {
                Ok(BuildProductObservation {
                    name: Alias::new(product.name.as_str())?,
                    artifact_identity: content_from_identity(&product.artifact_identity)?,
                    store_identity: content_from_identity(&product.store_identity)?,
                    store: None,
                })
            })
            .collect::<PortResult<Vec<_>>>()?,
        builder_identity: profile.runtime.builder_identity.clone(),
        worker_identity: content_from_identity(&contract.worker_identity)?,
        store_identity: content_from_identity(&contract.store_identity)?,
        cache_kind: match contract.cache.kind {
            CacheKind::None => BuildCacheKind::None,
            CacheKind::Local => BuildCacheKind::Local,
            CacheKind::Substitution => BuildCacheKind::Substitution,
        },
        cache_source_identity: contract
            .cache
            .source_identity
            .as_ref()
            .map(content_from_identity)
            .transpose()?,
        log_identities: contract
            .logs
            .iter()
            .map(content_from_identity)
            .collect::<PortResult<Vec<_>>>()?,
        metrics: contract
            .metrics
            .iter()
            .map(|metric| {
                Ok(BuildMetricObservation {
                    name: Alias::new(metric.name.as_str())?,
                    value: metric.value,
                })
            })
            .collect::<PortResult<Vec<_>>>()?,
        receipt_identity: contract
            .receipt_identity
            .as_ref()
            .map(content_from_identity)
            .transpose()?,
    })
}

fn metric(name: &str, value: u64) -> PortResult<MetricObservation> {
    Ok(MetricObservation {
        name: label(name, "mantle.report.metric.name")?,
        value,
    })
}

fn label(value: &str, field: &str) -> PortResult<Label> {
    Label::new(value.to_string()).map_err(|_| {
        domain(
            "mantle_label",
            field,
            "Mantle label does not fit the pinned contract",
        )
    })
}

fn identity_from_content(content: &ContentId) -> PortResult<Identity> {
    Identity::new(format!("b3:{}", content.as_str())).map_err(|_| {
        domain(
            "mantle_identity",
            "mantle.identity",
            "Kiln content identity does not fit the Mantle contract",
        )
    })
}

fn content_from_identity(identity: &Identity) -> PortResult<ContentId> {
    let value = identity.as_str().strip_prefix("b3:").ok_or_else(|| {
        domain(
            "mantle_identity",
            "mantle.identity",
            "Mantle identity is missing its BLAKE3 prefix",
        )
    })?;
    ContentId::from_blake3_hex(value).map_err(PortFailure::Domain)
}

fn text_identity(domain_tag: &str, value: &str) -> PortResult<Identity> {
    tagged_identity(domain_tag, &[value.as_bytes()])
}

fn tagged_identity(domain_tag: &str, fields: &[&[u8]]) -> PortResult<Identity> {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, domain_tag.as_bytes())?;
    for field in fields {
        frame(&mut hasher, field)?;
    }
    Identity::new(format!("b3:{}", hasher.finalize().to_hex())).map_err(|_| {
        domain(
            "mantle_identity",
            "mantle.identity",
            "Mantle identity input did not produce a canonical BLAKE3 identity",
        )
    })
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) -> PortResult<()> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        domain(
            "mantle_identity_length",
            "mantle.identity",
            "Mantle identity input length is not representable",
        )
    })?;
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn contract_error(_error: crate::wire::ContractError) -> PortFailure {
    domain(
        "mantle_contract",
        "mantle.contract",
        "Mantle request or observation failed contract admission",
    )
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
