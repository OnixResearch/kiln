use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
const NATIVE_METRIC_COUNT: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedBuild {
    pub observation: BuildEvaluationObservation,
    pub logs: BTreeMap<ContentId, PathBuf>,
}

#[derive(Debug, Deserialize)]
struct NativeBuildReport {
    schema: String,
    store_dir: String,
    counts: NativeCounts,
    outcomes: Vec<NativeOutcome>,
    failed: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NativeCounts {
    succeeded_total: u64,
    built_total: u64,
    cached_total: u64,
    failed_total: u64,
}

#[derive(Debug, Deserialize)]
struct NativeOutcome {
    cached: bool,
    log_file: Option<String>,
    outputs: Vec<NativeOutput>,
}

#[derive(Debug, Deserialize)]
struct NativeOutput {
    name: String,
    path: String,
    artifact_attestation: NativeAttestation,
    substitution: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NativeAttestation {
    logical_path: String,
    path: String,
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
    if report.schema != NATIVE_REPORT_SCHEMA
        || report.store_dir != profile.runtime.store_path.to_string_lossy()
    {
        return Err(domain(
            "mantle_native_report_binding",
            "mantle.report",
            "Mantle native report schema or store differs from the profile",
        ));
    }
    let receipt = tagged_identity(REPORT_RECEIPT_DOMAIN, &[bytes])?;
    let required = request
        .required_products
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();
    let mut products = BTreeMap::<String, ProductObservation>::new();
    let mut logs = BTreeMap::<ContentId, PathBuf>::new();
    let mut saw_cached = false;
    let mut saw_substitution = false;
    for outcome in &report.outcomes {
        saw_cached |= outcome.cached;
        if let Some(path) = &outcome.log_file {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(domain(
                    "mantle_log_path",
                    "mantle.report.outcomes.log_file",
                    "Mantle log path must be absolute",
                ));
            }
            let identity = tagged_identity(
                LOG_IDENTITY_DOMAIN,
                &[
                    receipt.as_str().as_bytes(),
                    path.as_os_str().as_encoded_bytes(),
                ],
            )?;
            logs.insert(content_from_identity(&identity)?, path);
        }
        for output in &outcome.outputs {
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
            saw_substitution |= output.substitution.is_some();
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
    if products.len() > profile.limits.max_products
        || logs.len() > profile.limits.max_logs
        || NATIVE_METRIC_COUNT > profile.limits.max_metrics
    {
        return Err(domain(
            "mantle_report_bound",
            "mantle.report",
            "Mantle native report exceeds a selected collection bound",
        ));
    }
    let successful = report.failed.is_empty() && report.counts.failed_total == 0;
    let outcome = if successful {
        BuildOutcome::Success
    } else {
        BuildOutcome::Failed
    };
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
        metrics: vec![
            metric("built-total", report.counts.built_total)?,
            metric("cached-total", report.counts.cached_total)?,
            metric("failed-total", report.counts.failed_total)?,
            metric("succeeded-total", report.counts.succeeded_total)?,
        ],
        receipt_identity: Some(receipt),
        non_claims: REQUIRED_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    };
    contract.identity = observation_identity(&contract);
    let observation = admitted_contract(profile, plan, request, &contract)?;
    Ok(NormalizedBuild { observation, logs })
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
        products: contract
            .products
            .iter()
            .map(|product| {
                Ok(BuildProductObservation {
                    name: Alias::new(product.name.as_str())?,
                    artifact_identity: content_from_identity(&product.artifact_identity)?,
                    store_identity: content_from_identity(&product.store_identity)?,
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
