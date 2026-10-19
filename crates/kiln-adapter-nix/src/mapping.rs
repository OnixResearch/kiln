use std::path::Path;

use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
    BuildProductObservation, admit_build_evaluation, validate_build_effect_plan,
};
use kiln_core::types::{ContentId, Diagnostic};
use kiln_provider_api::{PortFailure, PortResult};

use crate::profile::NixBuildProfile;
use crate::wire::{BuildRequest, REQUEST_SCHEMA, is_safe_store_path, parse_native_report};

const ARTIFACT_IDENTITY_DOMAIN: &str = "kiln.nix_build.artifact-reference.v1";
const STORE_OBJECT_IDENTITY_DOMAIN: &str = "kiln.nix_build.store-object-reference.v1";
const LOG_IDENTITY_DOMAIN: &str = "kiln.nix_build.log-reference.v1";
const NATIVE_RECEIPT_DOMAIN: &str = "kiln.nix_build.native-receipt.v1";
const SHELL_RECEIPT_DOMAIN: &str = "kiln.nix_build.shell-receipt.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedBuild {
    pub observation: BuildEvaluationObservation,
    pub log_identity: ContentId,
    pub log: Vec<u8>,
}

// r[impl kiln.nix_build.request]
pub(crate) fn lower_request(
    profile: &NixBuildProfile,
    plan: &BuildEffectPlan,
) -> PortResult<BuildRequest> {
    validate_build_effect_plan(plan, profile.limits.max_required_products)?;
    if plan.provider_profile != profile.provider.profile_id
        || plan.plan_identity != profile.runtime.plan_identity
        || plan.policy_identity != profile.runtime.policy_identity
        || plan.platform.as_str() != profile.platform
    {
        return Err(domain(
            "nix_request_profile",
            "nix.request",
            "Nix build plan differs from the admitted provider, plan, policy, or platform",
        ));
    }
    if plan.required_products != profile.runtime.required_products {
        return Err(domain(
            "nix_request_products",
            "nix.request.required_products",
            "Nix build plan products differ from the exact admitted derivation products",
        ));
    }
    let derivation_path = profile.runtime.derivation_path.to_str().ok_or_else(|| {
        domain(
            "nix_request_derivation",
            "nix.request.derivation_path",
            "Nix derivation path is not valid UTF-8",
        )
    })?;
    let request = BuildRequest {
        schema: REQUEST_SCHEMA.to_string(),
        derivation_path: derivation_path.to_string(),
        required_products: plan.required_products.clone(),
    };
    request
        .validate(profile.limits.max_required_products)
        .map_err(|_| {
            domain(
                "nix_request_contract",
                "nix.request",
                "Nix build request failed native contract admission",
            )
        })?;
    Ok(request)
}

// r[impl kiln.nix_build.completion]
pub(crate) fn normalize_native_report(
    profile: &NixBuildProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    stdout: &[u8],
    stderr: &[u8],
) -> PortResult<NormalizedBuild> {
    let log_identity = log_identity(stderr)?;
    let Ok(result) = parse_native_report(stdout, profile.limits.max_products) else {
        return shell_outcome(
            profile,
            plan,
            request,
            BuildEvaluationOutcome::ProviderError,
            stdout,
            stderr,
            log_identity,
        );
    };
    if result.drv_path != request.derivation_path {
        return shell_outcome(
            profile,
            plan,
            request,
            BuildEvaluationOutcome::ProviderError,
            stdout,
            stderr,
            log_identity,
        );
    }
    let mut products = Vec::with_capacity(plan.required_products.len());
    for product in &plan.required_products {
        let Some(path) = result.outputs.get(product.as_str()) else {
            return shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::ProviderError,
                stdout,
                stderr,
                log_identity,
            );
        };
        if !output_path_is_admitted(profile, path) {
            return shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::ProviderError,
                stdout,
                stderr,
                log_identity,
            );
        }
        products.push(BuildProductObservation {
            name: product.clone(),
            artifact_identity: content_identity(
                ARTIFACT_IDENTITY_DOMAIN,
                &[product.as_str().as_bytes(), path.as_bytes()],
            )?,
            store_identity: content_identity(STORE_OBJECT_IDENTITY_DOMAIN, &[path.as_bytes()])?,
        });
    }
    let receipt = content_identity(
        NATIVE_RECEIPT_DOMAIN,
        &[request.derivation_path.as_bytes(), stdout, stderr, b"0"],
    )?;
    let observation = observation(
        profile,
        plan,
        BuildEvaluationOutcome::Success,
        products,
        Some(receipt),
        log_identity.clone(),
    )?;
    Ok(NormalizedBuild {
        observation,
        log_identity,
        log: stderr.to_vec(),
    })
}

pub(crate) fn shell_outcome(
    profile: &NixBuildProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    outcome: BuildEvaluationOutcome,
    stdout: &[u8],
    stderr: &[u8],
    log_identity: ContentId,
) -> PortResult<NormalizedBuild> {
    let outcome_text = match outcome {
        BuildEvaluationOutcome::Failed => "failed",
        BuildEvaluationOutcome::Cancelled => "cancelled",
        BuildEvaluationOutcome::TimedOut => "timed_out",
        BuildEvaluationOutcome::ProviderError => "provider_error",
        BuildEvaluationOutcome::Unknown => "unknown",
        BuildEvaluationOutcome::Success => {
            return Err(domain(
                "nix_shell_success",
                "nix.shell",
                "Nix success requires one admitted native build report",
            ));
        }
    };
    let receipt = if outcome == BuildEvaluationOutcome::Unknown {
        None
    } else {
        Some(content_identity(
            SHELL_RECEIPT_DOMAIN,
            &[
                request.derivation_path.as_bytes(),
                plan.effect_id.as_str().as_bytes(),
                outcome_text.as_bytes(),
                stdout,
                stderr,
            ],
        )?)
    };
    let observation = observation(
        profile,
        plan,
        outcome,
        Vec::new(),
        receipt,
        log_identity.clone(),
    )?;
    Ok(NormalizedBuild {
        observation,
        log_identity,
        log: stderr.to_vec(),
    })
}

fn observation(
    profile: &NixBuildProfile,
    plan: &BuildEffectPlan,
    outcome: BuildEvaluationOutcome,
    products: Vec<BuildProductObservation>,
    receipt_identity: Option<ContentId>,
    log_identity: ContentId,
) -> PortResult<BuildEvaluationObservation> {
    let observation = BuildEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        plan_identity: plan.plan_identity.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        platform: plan.platform.clone(),
        outcome,
        products,
        builder_identity: profile.runtime.executable_identity.clone(),
        worker_identity: profile.runtime.worker_identity.clone(),
        store_identity: profile.runtime.store_identity.clone(),
        cache_kind: BuildCacheKind::None,
        cache_source_identity: None,
        log_identities: vec![log_identity],
        metrics: Vec::new(),
        receipt_identity,
    };
    admit_build_evaluation(plan, &observation)?;
    Ok(observation)
}

// r[impl kiln.nix_build.logs]
pub(crate) fn log_identity(stderr: &[u8]) -> PortResult<ContentId> {
    content_identity(LOG_IDENTITY_DOMAIN, &[stderr])
}

fn output_path_is_admitted(profile: &NixBuildProfile, path: &str) -> bool {
    is_safe_store_path(path)
        && Path::new(path).parent() == Some(profile.runtime.store_path.as_path())
}

pub(crate) fn content_identity(domain_tag: &str, fields: &[&[u8]]) -> PortResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, domain_tag.as_bytes())?;
    for field in fields {
        frame(&mut hasher, field)?;
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string()).map_err(PortFailure::Domain)
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) -> PortResult<()> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        domain(
            "nix_identity_length",
            "nix.identity",
            "Nix identity input length is not representable",
        )
    })?;
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
