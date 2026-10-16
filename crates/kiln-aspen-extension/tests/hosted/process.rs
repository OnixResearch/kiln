use std::io::Write;
use std::process::{Command, Stdio};

use kiln_adapter_aspen::profile::{AdmittedAspenRuntimeProfile, profile_document_fixture};
use kiln_adapter_aspen::protocol::{
    CallbackCommand, CallbackDecision, CallbackOutcome, CallbackState, decode_aspen_native_outcome,
    decode_callback_outcome,
};

use super::support::{
    OVERSIZED_INPUT_BYTES, PROCESS_FAILURE_EXIT_CODE, admitted, native_request_frame, request,
    submit_request, trigger,
};

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.conformance]
#[test]
fn separate_callback_process_accepts_canonical_input_and_rejects_bad_frames() {
    let profile = admitted();
    let (_runtime_profile_directory, runtime_profile) = runtime_profile_file();
    let request = request(
        &profile,
        CallbackState::initial(profile.profile.generation),
        CallbackCommand::Status,
        "process-status",
    );
    let bytes = native_request_frame(&profile, &request);
    let executable = env!("CARGO_BIN_EXE_kiln-aspen-extension");
    let output = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &bytes,
    );
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

    let malformed = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        b"not-preserves",
    );
    assert_eq!(malformed.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(malformed.stdout.is_empty());

    let oversized = vec![0; OVERSIZED_INPUT_BYTES];
    let overbound = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &oversized,
    );
    assert_eq!(overbound.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(overbound.stdout.is_empty());
}

// r[verify kiln.aspen_runtime.callback] r[verify kiln.aspen_runtime.conformance]
#[test]
fn native_host_frames_repeat_and_denials_release_no_effects() {
    let profile = admitted();
    let (_runtime_profile_directory, runtime_profile) = runtime_profile_file();
    let executable = env!("CARGO_BIN_EXE_kiln-aspen-extension");
    let accepted_request = submit_request(&profile, trigger(false));
    let accepted_frame = native_request_frame(&profile, &accepted_request);
    let first = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &accepted_frame,
    );
    let second = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &accepted_frame,
    );
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let accepted = decode_process_outcome(&profile, &first.stdout);
    assert_eq!(accepted.decision, CallbackDecision::Accepted);
    assert_eq!(accepted.effects.len(), 1);

    let rejected_request = submit_request(&profile, trigger(true));
    let rejected_frame = native_request_frame(&profile, &rejected_request);
    let rejected = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &rejected_frame,
    );
    assert!(rejected.status.success());
    let rejected = decode_process_outcome(&profile, &rejected.stdout);
    assert_eq!(rejected.decision, CallbackDecision::Rejected);
    assert!(rejected.effects.is_empty());

    let mut denied_request = accepted_request;
    denied_request.authority_ref = "substituted-authority".to_string();
    let denied_frame = native_request_frame(&profile, &denied_request);
    let denied = run_callback_process(
        executable,
        &runtime_profile,
        profile.profile_identity.as_str(),
        &denied_frame,
    );
    assert_eq!(denied.status.code(), Some(PROCESS_FAILURE_EXIT_CODE));
    assert!(denied.stdout.is_empty());
}

fn decode_process_outcome(profile: &AdmittedAspenRuntimeProfile, bytes: &[u8]) -> CallbackOutcome {
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

fn runtime_profile_file() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory =
        tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary runtime profile directory"));
    let path = directory.path().join("runtime-profile.json");
    let bytes = serde_json::to_vec(&profile_document_fixture())
        .unwrap_or_else(|_| unreachable!("runtime profile encoding"));
    std::fs::write(&path, bytes).unwrap_or_else(|_| unreachable!("runtime profile write"));
    (directory, path)
}

fn run_callback_process(
    executable: &str,
    runtime_profile: &std::path::Path,
    runtime_profile_identity: &str,
    input: &[u8],
) -> std::process::Output {
    let mut child = Command::new(executable)
        .args([
            "--runtime-profile",
            runtime_profile
                .to_str()
                .unwrap_or_else(|| unreachable!("runtime profile path is UTF-8")),
            "--runtime-profile-identity",
            runtime_profile_identity,
        ])
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
