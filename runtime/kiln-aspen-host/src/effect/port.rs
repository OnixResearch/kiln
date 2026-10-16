use kiln_adapter_aspen::profile::AspenPortPurpose;
use kiln_adapter_aspen::protocol::{
    HOSTED_PROVIDER_COMPLETION_SCHEMA, HostedProviderCompletion, encode_hosted_provider_completion,
};
use kiln_provider_api::PortFailure;
use molten::fabric::{CanonicalFabricPortBinding, FabricPortError, FabricPortResult};
use molten::system_extension::{
    FabricEffectPort, NativeCallbackValue, PortEffectOutput, TypedEffectRequest,
};

use super::LatticeEffectPort;

const EFFECT_OPERATION: &str = "kiln-provider-effect-v2";

impl FabricEffectPort for LatticeEffectPort {
    // r[impl kiln.aspen_host.lattice]
    fn route(
        &mut self,
        binding: &CanonicalFabricPortBinding,
        effect: &TypedEffectRequest,
    ) -> FabricPortResult<PortEffectOutput> {
        let transport = self
            .profile
            .profile
            .ports
            .iter()
            .find(|candidate| candidate.purpose == AspenPortPurpose::Transport)
            .ok_or_else(|| FabricPortError::capability("Aspen transport port is absent"))?;
        if effect.operation != EFFECT_OPERATION
            || binding.binding.key.port_id != transport.port_id
            || binding.binding.key.version != transport.version
            || effect.input_schema_ref != transport.input_schema
            || effect.output_schema_ref != transport.output_schema
        {
            return Err(FabricPortError::malformed(
                "Aspen workflow effect does not match the exact transport binding",
            ));
        }
        let plan = self.materialize_effect(effect).map_err(|error| {
            eprintln!("Kiln Aspen host effect materialization failed: {error:?}");
            error
        })?;
        let observation = self
            .route_workflow(binding, effect, &plan)
            .map_err(|error| {
                eprintln!("Kiln Aspen host workflow routing failed: {error:?}");
                error
            })?;
        let completion = HostedProviderCompletion {
            schema: HOSTED_PROVIDER_COMPLETION_SCHEMA.to_string(),
            effect_id: plan.effect_id.clone(),
            operation_id: plan.operation_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            request_identity: plan.request_identity.clone(),
            observation,
        };
        let bytes = encode_hosted_provider_completion(
            &completion,
            self.profile.profile.bounds.max_materialized_value_bytes,
        )
        .map_err(|error| {
            eprintln!("Kiln Aspen host provider completion encoding failed: {error}");
            FabricPortError::malformed(error.to_string())
        })?;
        let output_ref = molten::preserves_rail::content_ref_from_bytes(&bytes);
        Ok(PortEffectOutput {
            output_schema_ref: effect.output_schema_ref.clone(),
            output_ref: output_ref.clone(),
            materialized_output: Some(NativeCallbackValue {
                value_ref: output_ref,
                bytes,
            }),
        })
    }
}

pub(super) fn provider_failure_code(error: &PortFailure) -> &str {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code.as_str(),
        PortFailure::Infrastructure(failure)
        | PortFailure::Retryable(failure)
        | PortFailure::TimedOut(failure)
        | PortFailure::Cancelled(failure)
        | PortFailure::Unknown(failure) => failure.code.as_str(),
    }
}

pub(super) fn map_provider_failure(error: PortFailure) -> FabricPortError {
    match error {
        PortFailure::Domain(diagnostic) => {
            FabricPortError::malformed(format!("{}: {}", diagnostic.code, diagnostic.message))
        }
        PortFailure::Infrastructure(failure) | PortFailure::Retryable(failure) => {
            FabricPortError::transport(format!("{}: {}", failure.code, failure.message))
        }
        PortFailure::TimedOut(failure) => FabricPortError::Timeout {
            message: format!("{}: {}", failure.code, failure.message),
        },
        PortFailure::Cancelled(failure) => FabricPortError::Cancelled {
            message: format!("{}: {}", failure.code, failure.message),
        },
        PortFailure::Unknown(failure) => {
            FabricPortError::uncertain(format!("{}: {}", failure.code, failure.message))
        }
    }
}
