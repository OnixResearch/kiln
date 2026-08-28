use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::protocol::{
    CALLBACK_OUTCOME_SCHEMA, CallbackCommand, CallbackDecision, CallbackOutcome, CallbackRequest,
    HostedRunPhase, decode_callback_request, encode_callback_outcome,
};
use kiln_adapter_aspen::service::{CallbackInvocationError, HostedCallback};
use kiln_adapter_aspen::transport::canonical_trigger_identity;

mod decision;
mod native;

use decision::*;

#[derive(Clone, Debug)]
pub struct KilnAspenCallback {
    profile: AdmittedAspenRuntimeProfile,
}

impl KilnAspenCallback {
    #[must_use]
    pub const fn new(profile: AdmittedAspenRuntimeProfile) -> Self {
        Self { profile }
    }

    // r[impl kiln.aspen_runtime.callback] r[impl kiln.aspen_runtime.lifecycle]
    pub fn handle(&self, request: &CallbackRequest) -> Result<CallbackOutcome, String> {
        validate_request(&self.profile, request)?;
        let mut state = request.prior_state.clone();
        let prior_checkpoint_ref = checkpoint_ref(&state)?;
        state.revision = request.sequence;
        let (decision, effects, checkpoint_ref, diagnostics) = match &request.command {
            CallbackCommand::SubmitTrigger {
                trigger,
                trigger_identity,
                idempotency_key,
            } => {
                let observed_identity = canonical_trigger_identity(
                    trigger,
                    self.profile.profile.bounds.max_callback_input_bytes,
                )
                .map_err(|error| format!("{error:?}"))?;
                if &observed_identity != trigger_identity {
                    return Err(
                        "trigger identity does not match canonical trigger bytes".to_string()
                    );
                }
                if let Some(existing) = state.runs.get(trigger_identity.as_str()) {
                    let diagnostics = vec![format!(
                        "replay returns existing run {}",
                        existing.run_id.as_str()
                    )];
                    (CallbackDecision::Replay, Vec::new(), None, diagnostics)
                } else if state.draining
                    || state.stopped
                    || trigger.before.as_ref() == Some(&trigger.after)
                {
                    let run = hosted_run(trigger_identity, HostedRunPhase::Denied, None)?;
                    state
                        .runs
                        .insert(trigger_identity.as_str().to_string(), run);
                    (
                        CallbackDecision::Rejected,
                        Vec::new(),
                        None,
                        vec!["trigger denied before provider effect planning".to_string()],
                    )
                } else {
                    let effect =
                        workflow_effect(&self.profile, request, trigger_identity, idempotency_key)?;
                    let run = hosted_run(
                        trigger_identity,
                        HostedRunPhase::DispatchPending,
                        Some(effect.effect_id.clone()),
                    )?;
                    state
                        .runs
                        .insert(trigger_identity.as_str().to_string(), run);
                    state
                        .unresolved_effects
                        .insert(effect.effect_id.as_str().to_string(), effect.clone());
                    (CallbackDecision::Accepted, vec![effect], None, Vec::new())
                }
            }
            CallbackCommand::ApplyProviderObservation {
                effect_id,
                provider_profile,
                observation_ref,
                outcome,
            } => {
                let effect = state
                    .unresolved_effects
                    .get(effect_id.as_str())
                    .ok_or_else(|| {
                        "provider observation does not name an unresolved effect".to_string()
                    })?;
                if &effect.provider_profile != provider_profile {
                    return Err(
                        "provider observation profile does not match the planned effect"
                            .to_string(),
                    );
                }
                observation_ref
                    .validate()
                    .map_err(|error| format!("{}: {}", error.code, error.message))?;
                apply_effect_terminal(
                    &mut state,
                    effect_id,
                    observation_ref,
                    phase_for_outcome(*outcome),
                )?;
                (
                    CallbackDecision::Reconciled,
                    Vec::new(),
                    None,
                    vec![
                        "provider observation remains subject to Kiln outcome admission"
                            .to_string(),
                    ],
                )
            }
            CallbackCommand::CompleteEffect(completion) => {
                validate_completion(request.generation, completion)?;
                apply_effect_terminal(
                    &mut state,
                    &completion.effect_id,
                    &completion.observation_ref,
                    phase_for_terminal(completion.terminal),
                )?;
                (CallbackDecision::Reconciled, Vec::new(), None, Vec::new())
            }
            CallbackCommand::Checkpoint => {
                let checkpoint_ref = checkpoint_ref(&state)?;
                (
                    CallbackDecision::Checkpointed,
                    Vec::new(),
                    Some(checkpoint_ref),
                    Vec::new(),
                )
            }
            CallbackCommand::Recover {
                checkpoint_ref: supplied_checkpoint_ref,
            } => {
                supplied_checkpoint_ref
                    .validate()
                    .map_err(|error| format!("{}: {}", error.code, error.message))?;
                let expected = prior_checkpoint_ref;
                if supplied_checkpoint_ref != &expected {
                    return Err(
                        "recovery checkpoint does not match the supplied canonical state"
                            .to_string(),
                    );
                }
                (
                    CallbackDecision::Recovered,
                    Vec::new(),
                    Some(expected),
                    vec!["unresolved effects require exact operation reconciliation".to_string()],
                )
            }
            CallbackCommand::Status => (CallbackDecision::Status, Vec::new(), None, Vec::new()),
            CallbackCommand::Drain => {
                state.draining = true;
                (
                    CallbackDecision::Draining,
                    Vec::new(),
                    None,
                    vec!["new trigger effects are disabled while draining".to_string()],
                )
            }
            CallbackCommand::Shutdown => {
                if !state.draining || !state.unresolved_effects.is_empty() {
                    return Err(
                        "shutdown requires a draining service with no unresolved effects"
                            .to_string(),
                    );
                }
                state.stopped = true;
                (CallbackDecision::Stopped, Vec::new(), None, Vec::new())
            }
        };
        let status = status(&state);
        Ok(CallbackOutcome {
            schema: CALLBACK_OUTCOME_SCHEMA.to_string(),
            protocol_schema: self.profile.profile.extension_protocol_schema.clone(),
            generation: request.generation,
            operation_id: request.operation_id.clone(),
            decision,
            state,
            effects,
            checkpoint_ref,
            status,
            diagnostics,
        })
    }
}

impl HostedCallback for KilnAspenCallback {
    fn invoke(
        &mut self,
        request: &[u8],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, CallbackInvocationError> {
        let decoded = decode_callback_request(
            request,
            self.profile.profile.bounds.max_callback_input_bytes,
        )
        .map_err(|error| CallbackInvocationError::Denied(error.to_string()))?;
        let outcome = self
            .handle(&decoded)
            .map_err(CallbackInvocationError::Denied)?;
        encode_callback_outcome(&outcome, maximum_output_bytes)
            .map_err(|error| CallbackInvocationError::Denied(error.to_string()))
    }
}
