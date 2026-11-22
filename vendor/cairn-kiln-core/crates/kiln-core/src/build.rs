use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    Alias, AttemptId, CandidateId, ContentId, CoreResult, Diagnostic, EffectId, PipelineRevisionId,
    ProviderProfileId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildEffectPlan {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub idempotency_key: ContentId,
    pub candidate_id: CandidateId,
    pub pipeline_revision: PipelineRevisionId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub provider_profile: ProviderProfileId,
    pub platform: Alias,
    pub required_products: Vec<Alias>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildEvaluationOutcome {
    Success,
    Failed,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildCacheKind {
    None,
    Local,
    Substitution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildProductObservation {
    pub name: Alias,
    pub artifact_identity: ContentId,
    pub store_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildMetricObservation {
    pub name: Alias,
    pub value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildEvaluationObservation {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub idempotency_key: ContentId,
    pub candidate_id: CandidateId,
    pub pipeline_revision: PipelineRevisionId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub provider_profile: ProviderProfileId,
    pub platform: Alias,
    pub outcome: BuildEvaluationOutcome,
    pub products: Vec<BuildProductObservation>,
    pub builder_identity: ContentId,
    pub worker_identity: ContentId,
    pub store_identity: ContentId,
    pub cache_kind: BuildCacheKind,
    pub cache_source_identity: Option<ContentId>,
    pub log_identities: Vec<ContentId>,
    pub metrics: Vec<BuildMetricObservation>,
    pub receipt_identity: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildLogReadRequest {
    pub effect_id: EffectId,
    pub log_identity: ContentId,
    pub offset: usize,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildLogChunk {
    pub effect_id: EffectId,
    pub log_identity: ContentId,
    pub offset: usize,
    pub bytes: Vec<u8>,
    pub end_of_log: bool,
}

// r[impl kiln.mantle.request]
pub fn validate_build_effect_plan(
    plan: &BuildEffectPlan,
    maximum_required_products: usize,
) -> CoreResult<()> {
    if maximum_required_products == 0
        || plan.required_products.is_empty()
        || plan.required_products.len() > maximum_required_products
    {
        return Err(Diagnostic::new(
            "build_product_bound",
            "build.required_products",
            "build product count is empty or exceeds its named bound",
        ));
    }
    let mut previous = None;
    for product in &plan.required_products {
        if previous.is_some_and(|previous| previous >= product) {
            return Err(Diagnostic::new(
                "build_product_order",
                "build.required_products",
                "build products must be sorted and unique",
            ));
        }
        previous = Some(product);
    }
    Ok(())
}

// r[impl kiln.mantle.completion]
pub fn admit_build_evaluation(
    plan: &BuildEffectPlan,
    observation: &BuildEvaluationObservation,
) -> CoreResult<()> {
    if observation.effect_id != plan.effect_id
        || observation.attempt_id != plan.attempt_id
        || observation.idempotency_key != plan.idempotency_key
        || observation.candidate_id != plan.candidate_id
        || observation.pipeline_revision != plan.pipeline_revision
        || observation.plan_identity != plan.plan_identity
        || observation.policy_identity != plan.policy_identity
        || observation.provider_profile != plan.provider_profile
        || observation.platform != plan.platform
    {
        return Err(Diagnostic::new(
            "build_observation_mismatch",
            "build.observation",
            "build observation does not apply to the exact admitted plan",
        ));
    }
    if observation.outcome == BuildEvaluationOutcome::Success
        && (observation.products.len() != plan.required_products.len()
            || plan
                .required_products
                .iter()
                .zip(&observation.products)
                .any(|(required, product)| required != &product.name))
    {
        return Err(Diagnostic::new(
            "build_product_mismatch",
            "build.observation.products",
            "successful build does not contain every required product exactly once",
        ));
    }
    if observation.outcome == BuildEvaluationOutcome::Unknown {
        if observation.receipt_identity.is_some() {
            return Err(Diagnostic::new(
                "build_unknown_receipt",
                "build.observation.receipt_identity",
                "unknown build outcome must remain without a terminal receipt",
            ));
        }
    } else if observation.receipt_identity.is_none() {
        return Err(Diagnostic::new(
            "build_terminal_receipt",
            "build.observation.receipt_identity",
            "terminal build outcome requires a bounded receipt identity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;

    use super::*;

    const IDENTITY_HEX_BYTES: usize = 64;
    const MAXIMUM_PRODUCTS: usize = 8;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
            .unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> BuildEffectPlan {
        BuildEffectPlan {
            effect_id: EffectId::new("build-a").unwrap_or_else(|_| unreachable!()),
            attempt_id: AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!()),
            idempotency_key: content('1'),
            candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
            pipeline_revision: PipelineRevisionId::new("pipeline-a")
                .unwrap_or_else(|_| unreachable!()),
            plan_identity: content('2'),
            policy_identity: content('3'),
            provider_profile: ProviderProfileId::new("mantle-build-v1")
                .unwrap_or_else(|_| unreachable!()),
            platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
            required_products: vec![Alias::new("binary").unwrap_or_else(|_| unreachable!())],
        }
    }

    fn observation(plan: &BuildEffectPlan) -> BuildEvaluationObservation {
        BuildEvaluationObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision: plan.pipeline_revision.clone(),
            plan_identity: plan.plan_identity.clone(),
            policy_identity: plan.policy_identity.clone(),
            provider_profile: plan.provider_profile.clone(),
            platform: plan.platform.clone(),
            outcome: BuildEvaluationOutcome::Success,
            products: vec![BuildProductObservation {
                name: Alias::new("binary").unwrap_or_else(|_| unreachable!()),
                artifact_identity: content('4'),
                store_identity: content('5'),
            }],
            builder_identity: content('6'),
            worker_identity: content('7'),
            store_identity: content('8'),
            cache_kind: BuildCacheKind::None,
            cache_source_identity: None,
            log_identities: Vec::new(),
            metrics: Vec::new(),
            receipt_identity: Some(content('9')),
        }
    }

    #[test]
    fn exact_build_observation_is_admitted() {
        let plan = plan();
        let observation = observation(&plan);
        assert_eq!(validate_build_effect_plan(&plan, MAXIMUM_PRODUCTS), Ok(()));
        assert_eq!(admit_build_evaluation(&plan, &observation), Ok(()));
    }

    #[test]
    fn stale_candidate_and_missing_products_are_rejected() {
        let plan = plan();
        let mut stale = observation(&plan);
        stale.candidate_id = CandidateId::new("candidate-b").unwrap_or_else(|_| unreachable!());
        assert!(admit_build_evaluation(&plan, &stale).is_err());

        let mut missing = observation(&plan);
        missing.products.clear();
        assert!(admit_build_evaluation(&plan, &missing).is_err());
    }
}
