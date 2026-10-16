use kiln_core::types::{
    ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, ProviderProfileId,
};
use serde::{Deserialize, Serialize};

use crate::profile::{AdmittedAspenRuntimeProfile, AspenPortPurpose};
use crate::protocol::{AspenEffectKind, CALLBACK_EFFECT_SCHEMA, CallbackEffectPlan};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEffectClass {
    DurableState,
    ContentStore,
    BoundedProcess,
    TransportSession,
    Timer,
    Schedule,
    Evidence,
}

impl ProviderEffectClass {
    #[must_use]
    pub const fn aspen_kind(self) -> AspenEffectKind {
        match self {
            Self::DurableState => AspenEffectKind::DurableState,
            Self::ContentStore => AspenEffectKind::ContentStore,
            Self::BoundedProcess => AspenEffectKind::BoundedProcess,
            Self::TransportSession => AspenEffectKind::TransportSession,
            Self::Timer => AspenEffectKind::Timer,
            Self::Schedule => AspenEffectKind::Schedule,
            Self::Evidence => AspenEffectKind::Evidence,
        }
    }

    #[must_use]
    pub const fn purpose(self) -> AspenPortPurpose {
        match self {
            Self::DurableState => AspenPortPurpose::Durability,
            Self::ContentStore => AspenPortPurpose::Content,
            Self::BoundedProcess => AspenPortPurpose::Execution,
            Self::TransportSession => AspenPortPurpose::Transport,
            Self::Timer => AspenPortPurpose::Time,
            Self::Schedule => AspenPortPurpose::Scheduling,
            Self::Evidence => AspenPortPurpose::Evidence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectMappingInput {
    pub approved: bool,
    pub effect_id: EffectId,
    pub class: ProviderEffectClass,
    pub provider_profile: ProviderProfileId,
    pub operation_id: ExternalRef,
    pub idempotency_key: ExternalRef,
    pub request_identity: ContentId,
    pub generation: u64,
    pub authority_ref: String,
    pub resource_ref: String,
}

// r[impl kiln.aspen_runtime.effects] r[impl kiln.aspen_runtime.boundary]
pub fn map_provider_effect(
    profile: &AdmittedAspenRuntimeProfile,
    input: &EffectMappingInput,
) -> CoreResult<CallbackEffectPlan> {
    if !input.approved {
        return Err(Diagnostic::new(
            "aspen_effect_denied",
            "aspen.effect.approved",
            "Kiln denied the provider effect before Aspen routing",
        ));
    }
    if input.generation != profile.profile.generation {
        return Err(Diagnostic::new(
            "aspen_effect_stale_generation",
            "aspen.effect.generation",
            "provider effect does not name the active Aspen service generation",
        ));
    }
    input.operation_id.validate()?;
    input.idempotency_key.validate()?;
    if input.authority_ref != profile.profile.authority_ref
        || input.resource_ref != profile.profile.resource_ref
    {
        return Err(Diagnostic::new(
            "aspen_effect_authority_drift",
            "aspen.effect.authority",
            "provider effect authority or resource profile does not match the selected runtime",
        ));
    }
    let purpose = input.class.purpose();
    let Some(binding) = profile
        .profile
        .ports
        .iter()
        .find(|port| port.purpose == purpose)
    else {
        return Err(Diagnostic::new(
            "aspen_effect_port_unavailable",
            "aspen.effect.port",
            "selected Aspen runtime does not provide the exact required fabric port",
        ));
    };
    if binding.authority_ref != input.authority_ref || binding.resource_ref != input.resource_ref {
        return Err(Diagnostic::new(
            "aspen_effect_port_authority",
            "aspen.effect.port",
            "fabric port binding does not admit the effect authority and resources",
        ));
    }
    Ok(CallbackEffectPlan {
        schema: CALLBACK_EFFECT_SCHEMA.to_string(),
        effect_id: input.effect_id.clone(),
        kind: input.class.aspen_kind(),
        provider_profile: input.provider_profile.clone(),
        operation_id: input.operation_id.clone(),
        idempotency_key: input.idempotency_key.clone(),
        request_identity: input.request_identity.clone(),
        authority_ref: input.authority_ref.clone(),
        resource_ref: input.resource_ref.clone(),
        generation: input.generation,
    })
}

// r[impl kiln.aspen_runtime.effects]
pub fn admit_effect_plan(
    profile: &AdmittedAspenRuntimeProfile,
    plan: &CallbackEffectPlan,
) -> CoreResult<()> {
    if plan.schema != CALLBACK_EFFECT_SCHEMA {
        return Err(Diagnostic::new(
            "aspen_effect_schema",
            "aspen.effect.schema",
            "provider effect schema is unsupported",
        ));
    }
    let input = EffectMappingInput {
        approved: true,
        effect_id: plan.effect_id.clone(),
        class: class_from_kind(plan.kind),
        provider_profile: plan.provider_profile.clone(),
        operation_id: plan.operation_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        request_identity: plan.request_identity.clone(),
        generation: plan.generation,
        authority_ref: plan.authority_ref.clone(),
        resource_ref: plan.resource_ref.clone(),
    };
    let expected = map_provider_effect(profile, &input)?;
    if expected != *plan {
        return Err(Diagnostic::new(
            "aspen_effect_mapping_drift",
            "aspen.effect",
            "provider effect differs from its canonical exact-port mapping",
        ));
    }
    Ok(())
}

const fn class_from_kind(kind: AspenEffectKind) -> ProviderEffectClass {
    match kind {
        AspenEffectKind::DurableState => ProviderEffectClass::DurableState,
        AspenEffectKind::ContentStore => ProviderEffectClass::ContentStore,
        AspenEffectKind::BoundedProcess => ProviderEffectClass::BoundedProcess,
        AspenEffectKind::TransportSession => ProviderEffectClass::TransportSession,
        AspenEffectKind::Timer => ProviderEffectClass::Timer,
        AspenEffectKind::Schedule => ProviderEffectClass::Schedule,
        AspenEffectKind::Evidence => ProviderEffectClass::Evidence,
    }
}
