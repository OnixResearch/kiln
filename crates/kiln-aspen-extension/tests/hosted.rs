mod support;

use std::io::Write;
use std::process::{Command, Stdio};

use kiln_adapter_aspen::protocol::{
    CallbackCommand, CallbackDecision, CallbackState, decode_aspen_native_outcome,
    decode_callback_outcome,
};
use kiln_adapter_aspen::service::{LocalAspenService, LocalServiceFault, LocalServiceJournal};
use kiln_adapter_aspen::transport::{
    AspenServiceDecision, AspenServiceTransport, AspenTriggerClient, IngressSource,
    ServiceTransportOutcome, build_ingress,
};
use kiln_aspen_extension::KilnAspenCallback;
use kiln_provider_api::{PortFailure, TriggerIntakePort};

use support::{
    OVERSIZED_INPUT_BYTES, PROCESS_FAILURE_EXIT_CODE, admitted, native_request_frame, request,
    submit_request, trigger,
};

// r[verify kiln.aspen_runtime.ingress] r[verify kiln.aspen_runtime.lifecycle]
#[test]
fn local_service_accepts_rejects_replays_and_recovers_after_restart() {
    let profile = admitted();
    let journal = LocalServiceJournal::new(profile.profile.generation);
    let service = LocalAspenService::new(
        profile.clone(),
        journal.clone(),
        KilnAspenCallback::new(profile.clone()),
    );
    let mut radicle = AspenTriggerClient::new(profile.clone(), IngressSource::Radicle, service);
    let accepted = radicle
        .accept(trigger(false))
        .unwrap_or_else(|_| unreachable!("accepted"));
    assert_eq!(
        accepted.terminal,
        kiln_provider_api::TriggerTerminal::Unknown
    );
    let replay = radicle
        .accept(trigger(false))
        .unwrap_or_else(|_| unreachable!("replay"));
    assert_eq!(replay.run_id, accepted.run_id);

    let rejected = radicle
        .accept(trigger(true))
        .unwrap_or_else(|_| unreachable!("rejected"));
    assert_eq!(
        rejected.terminal,
        kiln_provider_api::TriggerTerminal::Denied
    );

    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("ingress"));
    let mut restarted = LocalAspenService::new(
        profile.clone(),
        journal,
        KilnAspenCallback::new(profile.clone()),
    );
    let replay_outcome = restarted.submit(&profile, &ingress);
    assert!(matches!(
        replay_outcome,
        ServiceTransportOutcome::Acknowledged(_)
    ));
    let state = restarted
        .journal()
        .callback_state()
        .unwrap_or_else(|_| unreachable!("state"));
    assert_eq!(state.runs.len(), 2);
}

// r[verify kiln.aspen_runtime.failure]
#[test]
fn post_accept_disconnect_is_unknown_until_exact_reconciliation() {
    let profile = admitted();
    let journal = LocalServiceJournal::new(profile.profile.generation);
    journal
        .set_next_fault(LocalServiceFault::DisconnectAfterCommit)
        .unwrap_or_else(|_| unreachable!("fault"));
    let service = LocalAspenService::new(
        profile.clone(),
        journal,
        KilnAspenCallback::new(profile.clone()),
    );
    let mut client = AspenTriggerClient::new(profile.clone(), IngressSource::Operator, service);
    assert!(matches!(
        client.accept(trigger(false)),
        Err(PortFailure::Unknown(_))
    ));
    let operation = client
        .last_operation()
        .cloned()
        .unwrap_or_else(|| unreachable!("operation"));
    let reconciled = client.transport_mut().reconcile(&profile, &operation);
    let ServiceTransportOutcome::Acknowledged(ack) = reconciled else {
        unreachable!("exact operation reconciles");
    };
    assert_eq!(ack.decision, AspenServiceDecision::Accepted);
}

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.conformance]
#[test]
fn separate_callback_process_accepts_canonical_input_and_rejects_bad_frames() {
    let profile = admitted();
    let request = request(
        &profile,
        CallbackState::initial(profile.profile.generation),
        CallbackCommand::Status,
        "process-status",
    );
    let bytes = native_request_frame(&profile, &request);
    let executable = env!("CARGO_BIN_EXE_kiln-aspen-extension");
    let output = run_callback_process(executable, &bytes);
    assert!(output.status.success());
    let native = decode_aspen_native_outcome(
        &output.stdout,
        profile.profile.bounds.max_callback_output_bytes,
        profile.profile.bounds.max_materialized_value_bytes,
        profile.profile.bounds.max_materialized_values,
    )
    .unwrap_or_else(|_| unreachable!("native decode"));
    let callback_output = native
        .outputs
        .first()
        .unwrap_or_else(|| unreachable!("callback output"));
    let outcome = decode_callback_outcome(
        &callback_output.bytes,
        profile.profile.bounds.max_materialized_value_bytes,
        profile.profile.bounds.max_effects,
    )
    .unwrap_or_else(|_| unreachable!("Kiln decode"));
    assert_eq!(outcome.decision, CallbackDecision::Status);
    assert_eq!(
        native
            .state
            .unwrap_or_else(|| unreachable!("native state"))
            .bytes,
        serde_json::to_vec(&outcome.state).unwrap_or_else(|_| unreachable!("state encoding")),
    );

    let malformed = run_callback_process(executable, b"not-preserves");
    assert_eq!(malformed.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(malformed.stdout.is_empty());

    let oversized = vec![0; OVERSIZED_INPUT_BYTES];
    let overbound = run_callback_process(executable, &oversized);
    assert_eq!(overbound.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(overbound.stdout.is_empty());
}

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.conformance]
#[test]
fn native_host_frames_repeat_and_denials_release_no_effects() {
    let profile = admitted();
    let executable = env!("CARGO_BIN_EXE_kiln-aspen-extension");
    let accepted_request = submit_request(&profile, trigger(false));
    let accepted_frame = native_request_frame(&profile, &accepted_request);
    let first = run_callback_process(executable, &accepted_frame);
    let second = run_callback_process(executable, &accepted_frame);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let accepted = decode_process_outcome(&profile, &first.stdout);
    assert_eq!(accepted.decision, CallbackDecision::Accepted);
    assert_eq!(accepted.effects.len(), 1);

    let rejected_request = submit_request(&profile, trigger(true));
    let rejected_frame = native_request_frame(&profile, &rejected_request);
    let rejected = run_callback_process(executable, &rejected_frame);
    assert!(rejected.status.success());
    let rejected = decode_process_outcome(&profile, &rejected.stdout);
    assert_eq!(rejected.decision, CallbackDecision::Rejected);
    assert!(rejected.effects.is_empty());

    let mut denied_request = accepted_request;
    denied_request.authority_ref = "substituted-authority".to_string();
    let denied_frame = native_request_frame(&profile, &denied_request);
    let denied = run_callback_process(executable, &denied_frame);
    assert_eq!(denied.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(denied.stdout.is_empty());
}

fn decode_process_outcome(
    profile: &kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile,
    bytes: &[u8],
) -> kiln_adapter_aspen::protocol::CallbackOutcome {
    let native = decode_aspen_native_outcome(
        bytes,
        profile.profile.bounds.max_callback_output_bytes,
        profile.profile.bounds.max_materialized_value_bytes,
        profile.profile.bounds.max_materialized_values,
    )
    .unwrap_or_else(|_| unreachable!("native process outcome"));
    let output = native
        .outputs
        .first()
        .unwrap_or_else(|| unreachable!("Kiln process output"));
    decode_callback_outcome(
        &output.bytes,
        profile.profile.bounds.max_materialized_value_bytes,
        profile.profile.bounds.max_effects,
    )
    .unwrap_or_else(|_| unreachable!("Kiln process outcome"))
}

fn run_callback_process(executable: &str, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(executable)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| unreachable!("callback process starts"));
    child
        .stdin
        .as_mut()
        .unwrap_or_else(|| unreachable!("stdin"))
        .write_all(input)
        .unwrap_or_else(|_| unreachable!("write"));
    child
        .wait_with_output()
        .unwrap_or_else(|_| unreachable!("wait"))
}
