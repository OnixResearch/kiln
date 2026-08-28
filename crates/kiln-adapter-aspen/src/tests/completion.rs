use kiln_core::domain::Outcome;
use kiln_core::types::{
    AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use kiln_core::workflow::{WorkflowObservation, WorkflowState};

use super::super::*;
use super::support::{TEST_MAXIMUM_BYTES, reference};

const PROVIDER_OBSERVATION_SEQUENCE: u64 = 1;

// r[verify kiln.aspen_host.completion.accepted]
// r[verify kiln.aspen_host.completion.rejected]
#[test]
fn hosted_provider_completion_maps_exact_terminal_and_rejects_linkage_drift() {
    let (effect, completion) = provider_completion_fixture();
    let bytes = encode_hosted_provider_completion(&completion, TEST_MAXIMUM_BYTES)
        .unwrap_or_else(|_| unreachable!("provider completion encoding"));
    let decoded = decode_hosted_provider_completion(&bytes, TEST_MAXIMUM_BYTES)
        .unwrap_or_else(|_| unreachable!("provider completion decoding"));
    assert_eq!(decoded, completion);
    assert_eq!(
        admit_hosted_provider_completion(&effect, &decoded),
        Ok(CallbackEffectTerminal::Success),
    );

    let mut wrong_provider = completion.clone();
    wrong_provider.provider_profile =
        ProviderProfileId::new("wrong-provider").unwrap_or_else(|_| unreachable!("wrong provider"));
    assert!(admit_hosted_provider_completion(&effect, &wrong_provider).is_err());

    let mut non_terminal = completion;
    non_terminal.observation.terminal = false;
    non_terminal.observation.successful = false;
    non_terminal.observation.outcome = Outcome::Pending;
    assert_eq!(
        admit_hosted_provider_completion(&effect, &non_terminal),
        Ok(CallbackEffectTerminal::Unknown),
    );
}

fn provider_completion_fixture() -> (CallbackEffectPlan, HostedProviderCompletion) {
    let request_identity = ContentId::from_blake3_hex(
        blake3::hash(b"provider-completion-trigger")
            .to_hex()
            .to_string(),
    )
    .unwrap_or_else(|_| unreachable!("request identity"));
    let effect_id = EffectId::new("hosted-effect").unwrap_or_else(|_| unreachable!("effect"));
    let provider_profile = ProviderProfileId::new(KILN_WORKFLOW_PROVIDER_PROFILE)
        .unwrap_or_else(|_| unreachable!("provider"));
    let effect = CallbackEffectPlan {
        schema: CALLBACK_EFFECT_SCHEMA.to_string(),
        effect_id: effect_id.clone(),
        kind: AspenEffectKind::TransportSession,
        provider_profile: provider_profile.clone(),
        operation_id: reference("molten", "transport-operation-blake3", "operation"),
        idempotency_key: reference("kiln", "trigger-idempotency-blake3", "idempotency"),
        request_identity: request_identity.clone(),
        authority_ref: "kiln-aspen-authority-v1".to_string(),
        resource_ref: "kiln-aspen-resources-v1".to_string(),
        generation: INITIAL_SERVICE_GENERATION,
    };
    let observation = WorkflowObservation {
        effect_id: effect_id.clone(),
        attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!("attempt")),
        candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!("candidate")),
        pipeline_revision_id: PipelineRevisionId::new("pipeline")
            .unwrap_or_else(|_| unreachable!("pipeline")),
        workflow_policy_id: PolicyId::new("policy").unwrap_or_else(|_| unreachable!("policy")),
        provider_profile: provider_profile.clone(),
        idempotency_key: request_identity.clone(),
        workflow: reference("lattice", "workflow", "workflow"),
        execution: reference("lattice", "execution", "execution"),
        sequence: PROVIDER_OBSERVATION_SEQUENCE,
        state: WorkflowState::Completed,
        terminal: true,
        successful: true,
        outcome: Outcome::Success,
        nodes: Vec::new(),
        logs: Vec::new(),
        receipt: None,
    };
    let completion = HostedProviderCompletion {
        schema: HOSTED_PROVIDER_COMPLETION_SCHEMA.to_string(),
        effect_id,
        operation_id: effect.operation_id.clone(),
        provider_profile,
        request_identity,
        observation,
    };
    (effect, completion)
}
