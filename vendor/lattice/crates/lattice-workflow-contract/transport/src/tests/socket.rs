use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::thread;

use super::*;

const TEST_CHANNEL_CAPACITY: usize = 1;
const TEST_TIMEOUT_MILLISECONDS: u64 = 10;

// r[verify lattice.workflow_interchange.transport_peer.scenario.admitted]
#[test]
fn unix_client_and_server_exchange_one_admitted_request() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let profile = transport_profile(user_id);
    let server_profile = profile.clone();
    let (sender, receiver) = mpsc::sync_channel(TEST_CHANNEL_CAPACITY);
    let server = thread::spawn(move || {
        sender.send(()).expect("fixture readiness signal is delivered");
        let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
        serve_listener(&listener, &server_profile, &mut service)
    });
    receiver.recv().expect("fixture server is ready");

    let endpoint = UnixEndpoint {
        socket_path: &socket_path,
        profile: &profile,
    };
    let response = UnixClient::new(endpoint).exchange(&dispatch_request()).expect("local exchange succeeds");
    let report = server.join().expect("fixture server joins").expect("fixture server succeeds");

    assert!(matches!(
        response.result,
        ExchangeResult::Success(success) if matches!(success.as_ref(), ExchangeSuccess::Accepted(_))
    ));
    assert_eq!(report.completed_exchanges, TEST_CONNECTIONS);
    assert_eq!(report.failed_exchanges, 0);
}

// r[verify lattice.workflow_interchange.transport_ambiguity.scenario.unknown]
#[test]
fn client_classifies_a_missing_response_as_unknown_after_write() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow-disconnect.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let profile = transport_profile(user_id);
    let maximum_frame_bytes = profile.limits.maximum_frame_bytes;
    let server = thread::spawn(move || {
        let (mut stream, _address) = listener.accept().expect("fixture connection is accepted");
        read_frame(&mut stream, maximum_frame_bytes).expect("fixture request is complete");
    });

    let endpoint = UnixEndpoint {
        socket_path: &socket_path,
        profile: &profile,
    };
    let result = UnixClient::new(endpoint).exchange(&dispatch_request());
    server.join().expect("fixture disconnect server joins");

    assert_eq!(result, Err(ClientFailure::UnknownAfterWrite));
}

#[test]
fn recovery_disconnect_remains_unknown_without_automatic_redispatch() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow-recovery-disconnect.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let profile = transport_profile(user_id);
    let maximum_frame_bytes = profile.limits.maximum_frame_bytes;
    let server = thread::spawn(move || {
        let (mut stream, _address) = listener.accept().expect("fixture connection is accepted");
        read_frame(&mut stream, maximum_frame_bytes).expect("fixture recovery request is complete");
    });
    let contract_profile = contract_profile();
    let request = workflow_request(&contract_profile);
    let recovery = new_request(ExchangeCommand::RecoverDispatch {
        profile: contract_profile,
        request,
    })
    .expect("recovery request is encodable");

    let endpoint = UnixEndpoint {
        socket_path: &socket_path,
        profile: &profile,
    };
    let result = UnixClient::new(endpoint).exchange(&recovery);
    server.join().expect("fixture recovery server joins");

    assert_eq!(result, Err(ClientFailure::UnknownAfterWrite));
}

#[test]
fn client_classifies_a_malformed_response_as_unknown_after_write() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow-invalid-response.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let profile = transport_profile(user_id);
    let maximum_frame_bytes = profile.limits.maximum_frame_bytes;
    let server = thread::spawn(move || {
        let (mut stream, _address) = listener.accept().expect("fixture connection is accepted");
        read_frame(&mut stream, maximum_frame_bytes).expect("fixture request is complete");
        let malformed = serde_json::json!({ "schema": "wrong" });
        let frame = encode_frame(&malformed, maximum_frame_bytes).expect("malformed response frame is bounded");
        write_frame(&mut stream, &frame).expect("malformed response is written");
    });

    let endpoint = UnixEndpoint {
        socket_path: &socket_path,
        profile: &profile,
    };
    let result = UnixClient::new(endpoint).exchange(&dispatch_request());
    server.join().expect("fixture malformed server joins");

    assert_eq!(result, Err(ClientFailure::UnknownAfterWrite));
}

// r[verify lattice.workflow_interchange.transport_bounds.scenario.timeout]
#[test]
fn server_deadline_stops_a_stalled_peer_before_handler_invocation() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow-timeout.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let mut profile = transport_profile(user_id);
    profile.limits.read_deadline_milliseconds = TEST_TIMEOUT_MILLISECONDS;
    let server_profile = profile.clone();
    let server = thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
        let report = serve_listener(&listener, &server_profile, &mut service)?;
        Ok::<_, ServerFailure>((report, service.into_handler().calls))
    });
    let stalled = UnixStream::connect(&socket_path).expect("stalled fixture connects");
    let (report, calls) = server.join().expect("fixture server joins").expect("fixture server succeeds");
    drop(stalled);

    assert_eq!(report.failed_exchanges, TEST_CONNECTIONS);
    assert_eq!(calls, 0);
}

// r[verify lattice.workflow_interchange.transport_peer.scenario.denied]
#[test]
fn unix_server_rejects_an_unlisted_peer_before_handler_invocation() {
    let directory = tempfile::tempdir().expect("temporary socket directory exists");
    let socket_path = directory.path().join("workflow-denied.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture listener binds");
    let user_id = nix::unistd::getuid().as_raw();
    let denied_user = user_id ^ u32::MAX;
    debug_assert_ne!(denied_user, user_id);
    let profile = transport_profile(denied_user);
    let server_profile = profile.clone();
    let server = thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(FixtureHandler { calls: 0 });
        let report = serve_listener(&listener, &server_profile, &mut service)?;
        Ok::<_, ServerFailure>((report, service.into_handler().calls))
    });

    let endpoint = UnixEndpoint {
        socket_path: &socket_path,
        profile: &profile,
    };
    let result = UnixClient::new(endpoint).exchange(&dispatch_request());
    let (report, calls) = server.join().expect("fixture server joins").expect("fixture server succeeds");

    assert!(matches!(result, Err(ClientFailure::UnknownAfterWrite)));
    assert_eq!(report.rejected_peers, TEST_CONNECTIONS);
    assert_eq!(calls, 0);
}
