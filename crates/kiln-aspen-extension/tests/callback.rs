mod support;

use kiln_adapter_aspen::protocol::{
    CALLBACK_COMPLETION_SCHEMA, CallbackCommand, CallbackDecision, CallbackEffectCompletion,
    CallbackEffectTerminal, CallbackState, HostedRunPhase,
};
use kiln_aspen_extension::KilnAspenCallback;

use support::{admitted, reference, request, submit_request, trigger};

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.effects]
#[test]
fn callback_accepts_valid_trigger_deterministically_and_denies_without_effects() {
    let profile = admitted();
    let callback = KilnAspenCallback::new(profile.clone());
    let accepted = submit_request(&profile, trigger(false));
    let first = callback
        .handle(&accepted)
        .unwrap_or_else(|_| unreachable!("accept"));
    let second = callback
        .handle(&accepted)
        .unwrap_or_else(|_| unreachable!("repeat"));
    assert_eq!(first, second);
    assert_eq!(first.decision, CallbackDecision::Accepted);
    assert_eq!(first.effects.len(), 1);
    assert_eq!(first.state.unresolved_effects.len(), 1);

    let denied = submit_request(&profile, trigger(true));
    let outcome = callback
        .handle(&denied)
        .unwrap_or_else(|_| unreachable!("domain denial is an outcome"));
    assert_eq!(outcome.decision, CallbackDecision::Rejected);
    assert!(outcome.effects.is_empty());
    assert!(outcome.state.unresolved_effects.is_empty());
    assert!(
        outcome
            .state
            .runs
            .values()
            .all(|run| run.phase == HostedRunPhase::Denied)
    );
}

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.failure]
#[test]
fn callback_rejects_stale_authority_and_duplicate_completion() {
    let profile = admitted();
    let callback = KilnAspenCallback::new(profile.clone());
    let mut stale = submit_request(&profile, trigger(false));
    stale.authority_ref = "wrong-authority".to_string();
    assert!(callback.handle(&stale).is_err());

    let accepted = callback
        .handle(&submit_request(&profile, trigger(false)))
        .unwrap_or_else(|_| unreachable!("accepted"));
    let effect = accepted
        .effects
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!("effect"));
    let completion = CallbackEffectCompletion {
        schema: CALLBACK_COMPLETION_SCHEMA.to_string(),
        effect_id: effect.effect_id.clone(),
        operation_id: effect.operation_id,
        generation: profile.profile.generation,
        terminal: CallbackEffectTerminal::Success,
        observation_ref: reference("lattice", "observation", "complete-1"),
    };
    let completion_request = request(
        &profile,
        accepted.state,
        CallbackCommand::CompleteEffect(completion.clone()),
        "complete-1",
    );
    let completed = callback
        .handle(&completion_request)
        .unwrap_or_else(|_| unreachable!("completion"));
    assert!(completed.state.unresolved_effects.is_empty());
    assert!(
        completed
            .state
            .runs
            .values()
            .all(|run| run.phase == HostedRunPhase::Completed)
    );

    let duplicate = request(
        &profile,
        completed.state,
        CallbackCommand::CompleteEffect(completion),
        "complete-duplicate",
    );
    assert!(callback.handle(&duplicate).is_err());
}

// r[verify kiln.aspen_runtime.lifecycle]
#[test]
fn callback_checkpoints_recovers_drains_and_stops_with_generation_fencing() {
    let profile = admitted();
    let callback = KilnAspenCallback::new(profile.clone());
    let state = CallbackState::initial(profile.profile.generation);
    let checkpointed = callback
        .handle(&request(
            &profile,
            state,
            CallbackCommand::Checkpoint,
            "checkpoint",
        ))
        .unwrap_or_else(|_| unreachable!("checkpoint"));
    let checkpoint_ref = checkpointed
        .checkpoint_ref
        .clone()
        .unwrap_or_else(|| unreachable!("checkpoint ref"));
    let recovered = callback
        .handle(&request(
            &profile,
            checkpointed.state,
            CallbackCommand::Recover { checkpoint_ref },
            "recover",
        ))
        .unwrap_or_else(|_| unreachable!("recover"));
    let drained = callback
        .handle(&request(
            &profile,
            recovered.state,
            CallbackCommand::Drain,
            "drain",
        ))
        .unwrap_or_else(|_| unreachable!("drain"));
    let stopped = callback
        .handle(&request(
            &profile,
            drained.state,
            CallbackCommand::Shutdown,
            "stop",
        ))
        .unwrap_or_else(|_| unreachable!("stop"));
    assert!(stopped.state.draining);
    assert!(stopped.state.stopped);

    let mut stale = request(&profile, stopped.state, CallbackCommand::Status, "stale");
    stale.generation = stale.generation.saturating_add(1);
    assert!(callback.handle(&stale).is_err());
}
