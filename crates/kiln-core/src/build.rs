use alloc::string::String;
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
    /// Optional typed sandbox constraints bound to this exact plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<BuildConstraints>,
}

/// Named bounds for the additive build facts below.
pub const MAX_BUILD_CONSTRAINT_PATHS: usize = 64;
pub const MAX_BUILD_STORE_REFERENCES: usize = 1024;
pub const MAX_BUILD_INPUTS: usize = 1024;
pub const MAX_BUILD_FACT_TEXT_BYTES: usize = 4096;

/// Provider-neutral constraints required before a build may start.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConstraints {
    pub system: Alias,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_memory_bytes: Option<u64>,
    /// Sorted, unique absolute paths that must be available read-only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_read_only_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub network_access: bool,
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub provide_bin_sh: bool,
}

/// Native NAR and store-object facts for one product. NAR SHA-256 is lowercase
/// hexadecimal and is deliberately separate from the Kiln BLAKE3 identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildStoreObjectObservation {
    pub nar_sha256: String,
    pub nar_size: u64,
    /// Sorted, unique absolute store paths referenced by the output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    /// True only when the candidate set covered the complete reference closure.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub references_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deriver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<String>,
}

/// One exact input store object observed for a build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInputObservation {
    pub path: String,
    pub nar_sha256: String,
    pub nar_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildProductObservation {
    pub name: Alias,
    pub artifact_identity: ContentId,
    pub store_identity: ContentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<BuildStoreObjectObservation>,
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
    /// Bound sandbox constraints when the provider observed them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<BuildConstraints>,
    /// Sorted, unique exact input store objects observed for this build.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<BuildInputObservation>,
    /// BLAKE3 over the canonical input facts, present exactly when inputs exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_closure_identity: Option<ContentId>,
    /// True only when the input list covers the complete observed closure.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub input_closure_complete: bool,
    /// Kiln identity over the admitted derivation structure, when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation_identity: Option<ContentId>,
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
    if let Some(constraints) = &plan.constraints {
        validate_build_constraints(constraints)?;
        if constraints.system != plan.platform {
            return Err(Diagnostic::new(
                "build_constraint_system",
                "build.constraints.system",
                "build constraint system differs from the plan platform",
            ));
        }
    }
    Ok(())
}

const fn default_true() -> bool {
    true
}

const fn is_true(value: &bool) -> bool {
    *value
}

fn safe_absolute_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BUILD_FACT_TEXT_BYTES
        && value.starts_with('/')
        && !value.contains('\0')
        && value
            .split('/')
            .skip(1)
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn nar_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_build_constraints(constraints: &BuildConstraints) -> CoreResult<()> {
    if constraints.min_memory_bytes == Some(0) {
        return Err(Diagnostic::new(
            "build_constraint_memory",
            "build.constraints.min_memory_bytes",
            "build minimum memory must be positive when present",
        ));
    }
    if constraints.available_read_only_paths.len() > MAX_BUILD_CONSTRAINT_PATHS {
        return Err(Diagnostic::new(
            "build_constraint_paths",
            "build.constraints.available_read_only_paths",
            "build read-only path count exceeds its named bound",
        ));
    }
    let mut previous: Option<&str> = None;
    for path in &constraints.available_read_only_paths {
        if !safe_absolute_path(path) {
            return Err(Diagnostic::new(
                "build_constraint_path",
                "build.constraints.available_read_only_paths",
                "build read-only path is not a safe absolute path",
            ));
        }
        if previous.is_some_and(|previous| previous >= path.as_str()) {
            return Err(Diagnostic::new(
                "build_constraint_path_order",
                "build.constraints.available_read_only_paths",
                "build read-only paths must be sorted and unique",
            ));
        }
        previous = Some(path);
    }
    Ok(())
}

fn validate_store_object(store: &BuildStoreObjectObservation) -> CoreResult<()> {
    if !nar_sha256(&store.nar_sha256) || store.nar_size == 0 {
        return Err(Diagnostic::new(
            "build_store_nar",
            "build.observation.products.store",
            "product NAR hash or size is invalid",
        ));
    }
    if store.references.len() > MAX_BUILD_STORE_REFERENCES {
        return Err(Diagnostic::new(
            "build_store_reference_bound",
            "build.observation.products.store.references",
            "product reference count exceeds its named bound",
        ));
    }
    let mut previous: Option<&str> = None;
    for reference in &store.references {
        if !safe_absolute_path(reference) {
            return Err(Diagnostic::new(
                "build_store_reference",
                "build.observation.products.store.references",
                "product reference is not a safe absolute store path",
            ));
        }
        if previous.is_some_and(|previous| previous >= reference.as_str()) {
            return Err(Diagnostic::new(
                "build_store_reference_order",
                "build.observation.products.store.references",
                "product references must be sorted and unique",
            ));
        }
        previous = Some(reference);
    }
    for value in store
        .deriver
        .iter()
        .chain(store.content_address.iter())
        .chain(store.signatures.iter())
    {
        if value.is_empty() || value.len() > MAX_BUILD_FACT_TEXT_BYTES {
            return Err(Diagnostic::new(
                "build_store_text",
                "build.observation.products.store",
                "product store text fact is empty or over its named bound",
            ));
        }
    }
    Ok(())
}

fn validate_inputs(observation: &BuildEvaluationObservation) -> CoreResult<()> {
    if observation.inputs.len() > MAX_BUILD_INPUTS {
        return Err(Diagnostic::new(
            "build_input_bound",
            "build.observation.inputs",
            "build input fact count exceeds its named bound",
        ));
    }
    let mut previous: Option<&str> = None;
    for input in &observation.inputs {
        if !safe_absolute_path(&input.path) || !nar_sha256(&input.nar_sha256) || input.nar_size == 0
        {
            return Err(Diagnostic::new(
                "build_input_fact",
                "build.observation.inputs",
                "build input path, NAR hash or size is invalid",
            ));
        }
        if previous.is_some_and(|previous| previous >= input.path.as_str()) {
            return Err(Diagnostic::new(
                "build_input_order",
                "build.observation.inputs",
                "build input facts must be sorted by unique path",
            ));
        }
        previous = Some(&input.path);
    }
    match (
        observation.inputs.is_empty(),
        observation.input_closure_identity.as_ref(),
    ) {
        (true, None) | (false, Some(_)) => Ok(()),
        _ => Err(Diagnostic::new(
            "build_input_closure",
            "build.observation.input_closure_identity",
            "input closure identity must be present exactly when input facts exist",
        )),
    }
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
    if observation.constraints != plan.constraints {
        return Err(Diagnostic::new(
            "build_constraint_mismatch",
            "build.observation.constraints",
            "build observation constraints differ from the admitted plan",
        ));
    }
    if let Some(constraints) = &observation.constraints {
        validate_build_constraints(constraints)?;
    }
    validate_inputs(observation)?;
    for product in &observation.products {
        if let Some(store) = &product.store {
            validate_store_object(store)?;
        }
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

    pub(super) fn plan_for_facts() -> BuildEffectPlan {
        plan()
    }

    pub(super) fn observation_for_facts(plan: &BuildEffectPlan) -> BuildEvaluationObservation {
        observation(plan)
    }

    pub(super) fn content(character: char) -> ContentId {
        content_identity(character)
    }

    fn content_identity(character: char) -> ContentId {
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
            constraints: None,
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
            constraints: None,
            inputs: Vec::new(),
            input_closure_identity: None,
            input_closure_complete: true,
            derivation_identity: None,
            products: vec![BuildProductObservation {
                name: Alias::new("binary").unwrap_or_else(|_| unreachable!()),
                artifact_identity: content('4'),
                store_identity: content('5'),
                store: None,
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

#[cfg(test)]
mod fact_tests {
    use super::*;
    use alloc::{format, string::ToString, vec};

    fn path(character: &str) -> String {
        format!("/nix/store/{}-{character}", "a".repeat(32))
    }

    fn constrained_plan() -> BuildEffectPlan {
        let mut plan = super::tests::plan_for_facts();
        plan.constraints = Some(BuildConstraints {
            system: plan.platform.clone(),
            min_memory_bytes: Some(1024),
            available_read_only_paths: vec!["/dev/kvm".into()],
            network_access: false,
            provide_bin_sh: true,
        });
        plan
    }

    // r[verify kiln.nix_store.contracts] r[verify kiln.nix_store.contracts.scenario.absent] r[verify kiln.nix_store.compat] r[verify kiln.nix_store.compat.scenario.old]
    #[test]
    fn absent_additive_fields_keep_their_bytes() {
        let plan = super::tests::plan_for_facts();
        let value = serde_json::to_value(&plan).unwrap_or_else(|_| unreachable!());
        assert!(value.get("constraints").is_none());
        let observation = super::tests::observation_for_facts(&plan);
        let value = serde_json::to_value(&observation).unwrap_or_else(|_| unreachable!());
        assert!(value.get("inputs").is_none());
        assert!(value.get("input_closure_identity").is_none());
        assert!(value.get("constraints").is_none());
        assert!(
            value
                .get("products")
                .and_then(serde_json::Value::as_array)
                .and_then(|products| products.first())
                .and_then(|product| product.get("store"))
                .is_none()
        );
    }

    // r[verify kiln.nix_store.constraints] r[verify kiln.nix_store.constraints.scenario.invalid]
    #[test]
    fn constraints_store_objects_and_inputs_are_admitted() {
        let plan = constrained_plan();
        let mut observation = super::tests::observation_for_facts(&plan);
        observation.constraints = plan.constraints.clone();
        observation
            .products
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .store = Some(BuildStoreObjectObservation {
            nar_sha256: "b".repeat(64),
            nar_size: 12,
            references: vec![path("ref")],
            references_complete: true,
            deriver: Some(path("deriver")),
            content_address: Some("fixed:r:sha256:".to_string() + &"c".repeat(52)),
            signatures: Vec::new(),
        });
        observation.inputs = vec![BuildInputObservation {
            path: path("input"),
            nar_sha256: "d".repeat(64),
            nar_size: 4,
        }];
        observation.input_closure_identity = Some(super::tests::content('a'));
        observation.input_closure_complete = true;
        assert_eq!(admit_build_evaluation(&plan, &observation), Ok(()));

        let mut bad = observation.clone();
        bad.inputs
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .path = "/tmp/../escape".into();
        assert!(admit_build_evaluation(&plan, &bad).is_err());
        let mut bad = observation.clone();
        bad.input_closure_identity = None;
        assert!(admit_build_evaluation(&plan, &bad).is_err());
        let mut bad = observation.clone();
        bad.products
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .store = Some(BuildStoreObjectObservation {
            nar_sha256: "not-a-hash".into(),
            nar_size: 0,
            references: Vec::new(),
            references_complete: true,
            deriver: None,
            content_address: None,
            signatures: Vec::new(),
        });
        assert!(admit_build_evaluation(&plan, &bad).is_err());
        let mut bad = constrained_plan();
        bad.constraints = Some(BuildConstraints {
            system: bad.platform.clone(),
            min_memory_bytes: Some(0),
            available_read_only_paths: Vec::new(),
            network_access: false,
            provide_bin_sh: false,
        });
        assert!(validate_build_effect_plan(&bad, 8).is_err());
    }
}
