use super::*;

const ACCEPTED_TEST_MAXIMUM_CONNECTIONS: u32 = 3;
const EXPECTED_OBSERVATION_WAITS: usize = 2;

// r[verify kiln.aspen_host.composition.accepted]
// r[verify kiln.aspen_host.lattice.accepted]
// r[verify kiln.aspen_host.lattice_poll_pacing.accepted]
// r[verify kiln.aspen_host.completion.accepted]
// r[verify kiln.aspen_host.nonclaims.accepted]
// r[verify kiln.aspen_host.transport.accepted]
// r[verify kiln.aspen_host.production_durability.accepted]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the live integration test keeps the complete bounded host lifecycle visible"
)]
fn real_aspen_host_routes_lattice_and_returns_terminal_kiln_ack() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let lattice_socket = directory.path().join("lattice.sock");
    let replay_path = directory.path().join("replay.sqlite");
    let listener =
        UnixListener::bind(&lattice_socket).unwrap_or_else(|_| unreachable!("Lattice listener"));
    let user_id = nix::unistd::getuid().as_raw();
    let mut config = radicle_config(lattice_socket.clone(), replay_path.clone(), user_id);
    config.transport_limits.maximum_connections = ACCEPTED_TEST_MAXIMUM_CONNECTIONS;
    let admitted_radicle = admit_config(config).unwrap_or_else(|_| unreachable!("Radicle profile"));
    let server_profile = admitted_radicle.transport.clone();
    let lattice_server = std::thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(TerminalHandler {
            pending_observations_remaining: DELAYED_OBSERVATION_COUNT,
        });
        serve_listener(&listener, &server_profile, &mut service)
    });
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("Aspen profile"));
    let transport = UnixWorkflowExchange::new(
        admitted_radicle.shell.socket_path.clone(),
        admitted_radicle.transport,
        admitted_radicle.lattice.contract.clone(),
        admitted_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("Lattice transport"));
    let lattice = LatticeAdapter::new(admitted_radicle.lattice, transport)
        .unwrap_or_else(|_| unreachable!("Lattice adapter"));
    let cohort = HostCohort::build(profile.clone(), &host_paths(&directory))
        .unwrap_or_else(|error| unreachable!("Aspen cohort: {error}"));
    let observation_pacer = Arc::new(CountingObservationPacer::default());
    let effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: admitted_radicle.shell,
        lattice,
        values: cohort.values.clone(),
        ledger: cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: observation_pacer.clone(),
    })
    .unwrap_or_else(|_| unreachable!("Lattice effect port"));
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)
        .unwrap_or_else(|_| unreachable!("Aspen host runtime starts"));
    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));
    let host_socket = directory.path().join("aspen.sock");
    let host_profile = profile.clone();
    let host_config = ServerConfig {
        socket_path: host_socket.clone(),
        maximum_frame_bytes: profile
            .profile
            .bounds
            .max_callback_input_bytes
            .max(profile.profile.bounds.max_callback_output_bytes),
        maximum_requests: HOST_MAXIMUM_REQUESTS,
        timeout: std::time::Duration::from_millis(HOST_TIMEOUT_MILLISECONDS),
    };
    let host_server = std::thread::spawn(move || serve(&host_profile, &mut runtime, &host_config));
    for _attempt in 0..SOCKET_READY_ATTEMPTS {
        if host_socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(
            SOCKET_READY_DELAY_MILLISECONDS,
        ));
    }
    assert!(host_socket.exists());
    let mut client = UnixAspenServiceTransport::new(
        host_socket,
        profile.profile.bounds.max_callback_input_bytes,
        std::time::Duration::from_millis(HOST_TIMEOUT_MILLISECONDS),
    )
    .unwrap_or_else(|_| unreachable!("Aspen Unix client"));

    let result = client.submit(&profile, &ingress);
    assert_eq!(observation_pacer.wait_count(), EXPECTED_OBSERVATION_WAITS);
    let ServiceTransportOutcome::Acknowledged(ack) = result else {
        unreachable!("terminal acknowledgement");
    };
    assert_eq!(ack.decision, AspenServiceDecision::Completed);
    let replay = client.submit(&profile, &ingress);
    let ServiceTransportOutcome::Acknowledged(replay_ack) = replay else {
        unreachable!("replay acknowledgement");
    };
    assert_eq!(replay_ack.run_id, ack.run_id);
    assert_eq!(replay_ack.receipt_ref, ack.receipt_ref);
    let reconciled = client.reconcile(&profile, &ingress.operation_id);
    let ServiceTransportOutcome::Acknowledged(reconciled_ack) = reconciled else {
        unreachable!("reconciled acknowledgement");
    };
    assert_eq!(reconciled_ack.run_id, ack.run_id);
    assert_eq!(reconciled_ack.receipt_ref, ack.receipt_ref);
    host_server
        .join()
        .unwrap_or_else(|_| unreachable!("Aspen server join"))
        .unwrap_or_else(|_| unreachable!("Aspen server result"));
    let report = lattice_server
        .join()
        .unwrap_or_else(|_| unreachable!("Lattice server join"))
        .unwrap_or_else(|_| unreachable!("Lattice server report"));
    assert_eq!(
        report.completed_exchanges,
        ACCEPTED_TEST_MAXIMUM_CONNECTIONS,
    );

    let recovered_radicle = admit_config(radicle_config(lattice_socket, replay_path, user_id))
        .unwrap_or_else(|_| unreachable!("recovered Radicle profile"));
    let recovered_transport = UnixWorkflowExchange::new(
        recovered_radicle.shell.socket_path.clone(),
        recovered_radicle.transport,
        recovered_radicle.lattice.contract.clone(),
        recovered_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("recovered Lattice transport"));
    let recovered_lattice = LatticeAdapter::new(recovered_radicle.lattice, recovered_transport)
        .unwrap_or_else(|_| unreachable!("recovered Lattice adapter"));
    let recovered_cohort = HostCohort::build(profile.clone(), &host_paths(&directory))
        .unwrap_or_else(|error| unreachable!("recovered Aspen cohort: {error}"));
    let recovered_effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: recovered_radicle.shell,
        lattice: recovered_lattice,
        values: recovered_cohort.values.clone(),
        ledger: recovered_cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
    .unwrap_or_else(|_| unreachable!("recovered Lattice effect port"));
    let mut recovered_runtime = KilnAspenHostRuntime::start(recovered_cohort, recovered_effects)
        .unwrap_or_else(|error| unreachable!("recovered Aspen runtime: {error}"));
    let ServiceTransportOutcome::Acknowledged(recovered_ack) =
        recovered_runtime.submit(&profile, &ingress)
    else {
        unreachable!("durable replay acknowledgement");
    };
    assert_eq!(recovered_ack.run_id, ack.run_id);
    assert_eq!(recovered_ack.receipt_ref, ack.receipt_ref);
    assert!(recovered_runtime.shutdown().is_ok());
}
