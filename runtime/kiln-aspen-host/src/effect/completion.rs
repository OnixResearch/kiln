use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::protocol::{
    HOSTED_PROVIDER_COMPLETION_SCHEMA, HostedProviderCompletion, encode_hosted_provider_completion,
};
use kiln_core::workflow::WorkflowObservation;
use molten::system_extension::{
    CanonicalEffectCompletion, NativeCallbackValue, SYSTEM_EXTENSION_EFFECT_COMPLETION_SCHEMA,
};

use crate::durability::DurableEffectIntent;
use crate::error::{HostBridgeError, Result};

const EFFECT_COMPLETION_RECORD: &str = "system-extension-effect-completion-v2";
const NATIVE_CALLBACK_VALUE_RECORD: &str = "native-callback-value-v2";
const EFFECT_COMPLETION_CHECKS: [&str; 5] = [
    "exact-bound-port-routed",
    "generation-correlated",
    "output-schema-matched",
    "materialized-output-identity-checked",
    "completion-is-not-durability-proof",
];

pub(super) fn recovered_effect_completion(
    profile: &AdmittedAspenRuntimeProfile,
    intent: &DurableEffectIntent,
    observation: WorkflowObservation,
) -> Result<CanonicalEffectCompletion> {
    if !observation.terminal || observation.effect_id != intent.plan.effect_id {
        return Err(HostBridgeError::State(
            "recovered provider observation is not the exact terminal effect".to_string(),
        ));
    }
    let completion = HostedProviderCompletion {
        schema: HOSTED_PROVIDER_COMPLETION_SCHEMA.to_string(),
        effect_id: intent.plan.effect_id.clone(),
        operation_id: intent.plan.operation_id.clone(),
        provider_profile: intent.plan.provider_profile.clone(),
        request_identity: intent.plan.request_identity.clone(),
        observation,
    };
    let bytes = encode_hosted_provider_completion(
        &completion,
        profile.profile.bounds.max_materialized_value_bytes,
    )
    .map_err(|error| HostBridgeError::Protocol(error.to_string()))?;
    let output_ref = molten::preserves_rail::content_ref_from_bytes(&bytes);
    let materialized_output = NativeCallbackValue {
        value_ref: output_ref.clone(),
        bytes,
    };
    let value = completion_value(intent, &output_ref, &materialized_output);
    let completion_ref = molten::preserves_rail::canonical_hash(&value)
        .map_err(|error| HostBridgeError::State(error.to_string()))?;
    Ok(CanonicalEffectCompletion {
        completion_ref,
        callback_receipt_ref: intent.callback_receipt_ref.clone(),
        binding_ref: intent.binding_ref.clone(),
        request_ref: intent.request_ref.clone(),
        generation: intent.generation,
        output_ref,
        materialized_output: Some(materialized_output),
        value,
    })
}

fn completion_value(
    intent: &DurableEffectIntent,
    output_ref: &str,
    materialized_output: &NativeCallbackValue,
) -> preserves::IOValue {
    let field = |label: &'static str, value| molten::preserves_rail::record(label, vec![value]);
    molten::preserves_rail::record(
        EFFECT_COMPLETION_RECORD,
        vec![
            molten::preserves_rail::string(SYSTEM_EXTENSION_EFFECT_COMPLETION_SCHEMA),
            field(
                "callback-receipt-ref",
                molten::preserves_rail::string(&intent.callback_receipt_ref),
            ),
            field(
                "binding-ref",
                molten::preserves_rail::string(&intent.binding_ref),
            ),
            field("port-id", molten::preserves_rail::string(&intent.port_id)),
            field(
                "port-version",
                molten::preserves_rail::string(&intent.port_version),
            ),
            field(
                "request-ref",
                molten::preserves_rail::string(&intent.request_ref),
            ),
            field(
                "generation",
                molten::preserves_rail::u64_value(intent.generation),
            ),
            field(
                "output-schema-ref",
                molten::preserves_rail::string(&intent.output_schema_ref),
            ),
            field("output-ref", molten::preserves_rail::string(output_ref)),
            field("output-value", materialized_value(materialized_output)),
            checks_value(),
        ],
    )
}

fn materialized_value(value: &NativeCallbackValue) -> preserves::IOValue {
    molten::preserves_rail::record(
        "some",
        vec![molten::preserves_rail::record(
            NATIVE_CALLBACK_VALUE_RECORD,
            vec![
                molten::preserves_rail::string(&value.value_ref),
                molten::preserves_rail::sequence(
                    value
                        .bytes
                        .iter()
                        .map(|byte| molten::preserves_rail::u64_value(u64::from(*byte)))
                        .collect(),
                ),
            ],
        )],
    )
}

fn checks_value() -> preserves::IOValue {
    molten::preserves_rail::record(
        "checks",
        vec![molten::preserves_rail::sequence(
            EFFECT_COMPLETION_CHECKS
                .iter()
                .copied()
                .map(molten::preserves_rail::string)
                .collect(),
        )],
    )
}
